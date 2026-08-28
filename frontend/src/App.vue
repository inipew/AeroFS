<template>
  <div class="h-[100dvh] w-screen overflow-hidden flex flex-col bg-white dark:bg-[#0b0f19] text-gray-800 dark:text-slate-100 font-sans select-none antialiased fixed inset-0">
    <!-- Unauthenticated Login View -->
    <LoginModal v-if="!authStore.isAuthenticated && !authStore.isChecking" />

    <!-- Authenticated Main Application Workspace Surface -->
    <div v-else-if="authStore.isAuthenticated" class="h-full w-full flex flex-col min-h-0 overflow-hidden relative">
      <!-- Universal App Header -->
      <AppHeader
        @open-connection-dialog="isConnDialogOpen = true"
        @open-search-dialog="isSearchDialogOpen = true"
        @open-settings-dialog="isSettingsDialogOpen = true"
        @open-shares-dialog="isSharesDialogOpen = true"
        @open-trash-dialog="isTrashDialogOpen = true"
        @open-starred-dialog="isStarredDialogOpen = true"
        @open-archive-dialog="handleOpenArchive"
        @recent-view="handleRecentView"
      />

      <!-- Workspace Body (Sidebar + Content Workspace Area) -->
      <div class="flex-1 flex min-h-0 overflow-hidden relative">
        <!-- Collapsible / Floating Navigation Sidebar -->
        <AppSidebar
          @open-connection-dialog="isConnDialogOpen = true"
          @open-settings-dialog="isSettingsDialogOpen = true"
          @open-shares-dialog="isSharesDialogOpen = true"
          @open-trash-dialog="isTrashDialogOpen = true"
          @open-starred-dialog="isStarredDialogOpen = true"
        />

        <!-- Main Workspace Workspace Area -->
        <div class="flex-1 flex flex-col min-w-0 overflow-hidden relative bg-white dark:bg-[#0b0f19]">
          <!-- Mobile Dual Panel Switching Tabs (Top Navigation for Small Screens) -->
          <div
            v-if="uiStore.isMobile && workspaceStore.isDualPane"
            class="h-10 bg-gray-100/90 dark:bg-[#080c14]/90 border-b border-gray-200/80 dark:border-slate-800/80 px-3 flex items-center shrink-0 z-10 select-none"
          >
            <div class="w-full flex items-center bg-gray-200/70 dark:bg-slate-900/70 p-0.5 rounded-xl">
              <!-- Left Panel Tab -->
              <button
                @click="workspaceStore.setActivePanel('left')"
                :class="[
                  'flex-1 py-1.5 px-3 rounded-xl flex items-center justify-center space-x-1.5 transition-colors duration-fast text-xs font-semibold cursor-pointer truncate',
                  workspaceStore.activePanelId === 'left'
                    ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-xs ring-1 ring-blue-500/20'
                    : 'text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200 hover:bg-white/30 dark:hover:bg-slate-800/40'
                ]"
              >
                <FbIcon
                  :name="workspaceStore.leftPanel.connectionId === 'local' ? 'folder' : 'share'"
                  size="14px"
                  :class="workspaceStore.activePanelId === 'left' ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400'"
                />
                <span class="truncate max-w-[100px]">{{ leftConnName }}</span>
                <span class="text-[10px] font-mono opacity-60 truncate max-w-[65px]">/{{ getPanelDisplayPath(workspaceStore.leftPanel.path) }}</span>
                <span v-if="workspaceStore.activePanelId === 'left'" class="w-1.5 h-1.5 rounded-full bg-blue-500 shrink-0"></span>
              </button>

              <!-- Right Panel Tab -->
              <button
                @click="workspaceStore.setActivePanel('right')"
                :class="[
                  'flex-1 py-1.5 px-3 rounded-xl flex items-center justify-center space-x-1.5 transition-colors duration-fast text-xs font-semibold cursor-pointer truncate',
                  workspaceStore.activePanelId === 'right'
                    ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-xs ring-1 ring-blue-500/20'
                    : 'text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200 hover:bg-white/30 dark:hover:bg-slate-800/40'
                ]"
              >
                <FbIcon
                  :name="workspaceStore.rightPanel.connectionId === 'local' ? 'folder' : 'share'"
                  size="14px"
                  :class="workspaceStore.activePanelId === 'right' ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400'"
                />
                <span class="truncate max-w-[100px]">{{ rightConnName }}</span>
                <span class="text-[10px] font-mono opacity-60 truncate max-w-[65px]">/{{ getPanelDisplayPath(workspaceStore.rightPanel.path) }}</span>
                <span v-if="workspaceStore.activePanelId === 'right'" class="w-1.5 h-1.5 rounded-full bg-blue-500 shrink-0"></span>
              </button>
            </div>
          </div>

          <!-- Dynamic Workspace Shell (Continuous Surface) -->
          <main
            ref="mainContainerRef"
            :style="{ '--split-ratio': workspaceStore.splitRatio }"
            class="flex-1 flex overflow-hidden min-w-0 bg-white dark:bg-[#0b0f19] p-0 relative"
          >
            <!-- MOBILE VIEW: Continuous Dual-Slide Track with Real-Time Interactive Gesture Tracking -->
            <div
              v-if="uiStore.isMobile"
              ref="mobileTrackWrapperRef"
              class="w-full h-full flex flex-col min-w-0 overflow-hidden relative touch-pan-y"
              @touchstart="handleTouchStart"
              @touchmove="handleTouchMove"
              @touchend="handleTouchEnd"
              @touchcancel="handleTouchCancel"
            >
              <!-- Dual Slide Container when Dual-Pane is enabled -->
              <div
                v-if="workspaceStore.isDualPane"
                class="w-[200%] h-full flex flex-row flex-nowrap will-change-transform"
                :style="mobileTrackStyle"
              >
                <div class="w-1/2 h-full flex flex-col min-w-0 overflow-hidden shrink-0">
                  <FilePanel panelId="left" @open-archive-viewer="handleOpenArchiveViewer" />
                </div>
                <div class="w-1/2 h-full flex flex-col min-w-0 overflow-hidden shrink-0">
                  <FilePanel panelId="right" @open-archive-viewer="handleOpenArchiveViewer" />
                </div>
              </div>

              <!-- Single Panel View on Mobile when Dual-Pane is disabled -->
              <div v-else class="w-full h-full flex flex-col min-w-0 overflow-hidden">
                <FilePanel panelId="left" @open-archive-viewer="handleOpenArchiveViewer" />
              </div>
            </div>

            <!-- DESKTOP VIEW: Continuous Workspace Surface (Single or Split) -->
            <template v-else>
              <!-- Left Panel -->
              <div
                :style="{
                  width: workspaceStore.isDualPane ? 'calc(var(--split-ratio) * 100% - 3px)' : '100%'
                }"
                class="h-full flex flex-col min-w-[200px]"
              >
                <FilePanel panelId="left" @open-archive-viewer="handleOpenArchiveViewer" />
              </div>

              <!-- Draggable Split Divider (Compositor-Synced 1px seam with subtle hover handle) -->
              <div
                v-if="workspaceStore.isDualPane"
                @mousedown="startSplitResize"
                class="w-1.5 relative flex items-center justify-center cursor-col-resize hover:bg-blue-500/20 active:bg-blue-500/30 group select-none transition-colors duration-fast shrink-0 bg-gray-200 dark:border-slate-800 bg-gray-100 dark:bg-[#070b14]"
                title="Drag to resize panels"
              >
                <div class="w-0.5 h-10 bg-gray-300 dark:bg-slate-700 group-hover:bg-blue-500 rounded-full transition-colors duration-fast"></div>
              </div>

              <!-- Right Panel (when Dual Pane is enabled) -->
              <div
                v-if="workspaceStore.isDualPane"
                :style="{
                  width: 'calc((1 - var(--split-ratio)) * 100% - 3px)'
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
            class="min-h-[56px] h-14 bg-white/95 dark:bg-[#090d16]/95 backdrop-blur-md border-t border-gray-200 dark:border-slate-800 px-2 sm:px-4 flex items-center justify-around text-gray-500 dark:text-slate-400 text-[10px] font-semibold shrink-0 z-30 pb-safe select-none"
          >
            <button
              @click="workspaceStore.setActivePanel('left')"
              :class="[
                'flex-1 flex flex-col items-center justify-center py-1 space-y-1 transition-colors duration-fast cursor-pointer min-h-[44px]',
                workspaceStore.activePanelId === 'left' ? 'text-blue-600 dark:text-blue-400 font-bold' : 'hover:text-gray-900 dark:hover:text-slate-200'
              ]"
            >
              <FbIcon name="folder" size="18px" />
              <span>Left</span>
            </button>

            <button
              v-if="workspaceStore.isDualPane"
              @click="workspaceStore.setActivePanel('right')"
              :class="[
                'flex-1 flex flex-col items-center justify-center py-1 space-y-1 transition-colors duration-fast cursor-pointer min-h-[44px]',
                workspaceStore.activePanelId === 'right' ? 'text-blue-600 dark:text-blue-400 font-bold' : 'hover:text-gray-900 dark:hover:text-slate-200'
              ]"
            >
              <FbIcon name="folder" size="18px" />
              <span>Right</span>
            </button>

            <button
              @click="isConnDialogOpen = true"
              class="flex-1 flex flex-col items-center justify-center py-1 space-y-1 hover:text-blue-600 dark:hover:text-blue-400 transition-colors duration-fast cursor-pointer min-h-[44px]"
            >
              <FbIcon name="share" size="18px" />
              <span>Storage</span>
            </button>

            <button
              @click="transferStore.isDrawerOpen = !transferStore.isDrawerOpen"
              class="flex-1 flex flex-col items-center justify-center py-1 space-y-1 hover:text-blue-600 dark:hover:text-blue-400 transition-colors duration-fast cursor-pointer relative min-h-[44px]"
            >
              <div class="relative inline-flex items-center justify-center">
                <FbIcon name="refresh" size="18px" />
                <span
                  v-if="transferStore.activeCount > 0"
                  class="absolute -top-1 -right-1 w-2 h-2 rounded-full bg-blue-600 animate-pulse"
                ></span>
              </div>
              <span>Transfers</span>
            </button>

            <button
              @click="isSettingsDialogOpen = true"
              class="flex-1 flex flex-col items-center justify-center py-1 space-y-1 hover:text-blue-600 dark:hover:text-blue-400 transition-colors duration-fast cursor-pointer min-h-[44px]"
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

        <!-- Toast Notifications Container with Apple Spring Transitions -->
        <div class="fixed bottom-5 right-5 z-[9999] flex flex-col space-y-2 pointer-events-none max-w-sm w-full">
          <TransitionGroup
            enter-active-class="transition duration-[280ms] [transition-timing-function:var(--ease-spring)] transform"
            enter-from-class="translate-y-3 scale-95 opacity-0 sm:translate-y-0 sm:translate-x-3"
            enter-to-class="translate-y-0 scale-100 opacity-100 sm:translate-x-0"
            leave-active-class="transition duration-[180ms] [transition-timing-function:var(--ease-apple-in)] transform"
            leave-from-class="opacity-100 scale-100"
            leave-to-class="opacity-0 scale-95"
          >
            <div
              v-for="toast in uiStore.toasts"
              :key="toast.id"
              :class="[
                'pointer-events-auto flex items-center space-x-3 px-4 py-3 rounded-2xl shadow-2xl backdrop-blur-xl border text-xs font-semibold ring-1 ring-black/5 dark:ring-white/10 transition-[opacity,transform,background-color,border-color] duration-standard ease-spring',
                toast.type === 'success' ? 'bg-emerald-50/95 dark:bg-emerald-950/90 border-emerald-200 dark:border-emerald-800/80 text-emerald-900 dark:text-emerald-200' : '',
                toast.type === 'error' ? 'bg-rose-50/95 dark:bg-rose-950/90 border-rose-200 dark:border-rose-800/80 text-rose-900 dark:text-rose-200' : '',
                toast.type === 'warning' ? 'bg-amber-50/95 dark:bg-amber-950/90 border-amber-200 dark:border-amber-800/80 text-amber-900 dark:text-amber-200' : '',
                toast.type === 'info' ? 'bg-blue-50/95 dark:bg-slate-900/90 border-blue-200 dark:border-slate-800/80 text-gray-900 dark:text-slate-100' : ''
              ]"
            >
              <div
                class="w-6 h-6 rounded-full flex items-center justify-center shrink-0"
                :class="[
                  toast.type === 'success' ? 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-400' : '',
                  toast.type === 'error' ? 'bg-rose-500/20 text-rose-600 dark:text-rose-400' : '',
                  toast.type === 'warning' ? 'bg-amber-500/20 text-amber-600 dark:text-amber-400' : '',
                  toast.type === 'info' ? 'bg-blue-500/20 text-blue-600 dark:text-blue-400' : ''
                ]"
              >
                <FbIcon v-if="toast.type === 'success'" name="check" size="13px" />
                <FbIcon v-else-if="toast.type === 'error'" name="x" size="13px" />
                <FbIcon v-else-if="toast.type === 'warning'" name="info" size="13px" />
                <FbIcon v-else name="info" size="13px" />
              </div>
              <span class="flex-1 leading-snug">{{ toast.message }}</span>
            </div>
          </TransitionGroup>
        </div>

        <!-- Floating Transfer Engine Manager -->
        <TransferDrawer />
      </div>
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
import { ref, computed, onMounted, onUnmounted } from 'vue';
import FbIcon from './components/common/FbIcon.vue';
import { useAuthStore } from './stores/authStore';
import { useConnectionStore } from './stores/connectionStore';
import { useWorkspaceStore } from './stores/workspaceStore';
import { useTransferStore } from './stores/transferStore';
import { useFileStore } from './stores/fileStore';
import { useUiStore } from './stores/uiStore';
import { usePreferencesStore } from './stores/preferencesStore';
import { initializeCommandRegistry, commandRegistry } from './services/commandRegistry';
import { PreviewResolver } from './services/previewResolver';
import { getDynamicSettleDuration } from './motion/tokens';
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
const preferencesStore = usePreferencesStore();

const leftConnName = computed(() => {
  const conn = connStore.connections.find((c) => c.id === workspaceStore.leftPanel.connectionId);
  return conn?.name || (workspaceStore.leftPanel.connectionId === 'local' ? 'Local' : workspaceStore.leftPanel.connectionId);
});

const rightConnName = computed(() => {
  const conn = connStore.connections.find((c) => c.id === workspaceStore.rightPanel.connectionId);
  return conn?.name || (workspaceStore.rightPanel.connectionId === 'local' ? 'Local' : workspaceStore.rightPanel.connectionId);
});

function getPanelDisplayPath(path: string): string {
  if (!path || path === '/') return '';
  const parts = path.split('/').filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : '';
}

const mainContainerRef = ref<HTMLElement | null>(null);
let isResizingSplit = false;
let splitResizeRafId: number | null = null;
let currentPendingRatio = 0.5;

// ==========================================================================
// Mobile Real-Time Gesture Tracking with Apple Spring Physics & Rubber-Banding
// ==========================================================================
const isDraggingMobileTrack = ref(false);
const dragDeltaX = ref(0);
const mobileTrackWrapperRef = ref<HTMLElement | null>(null);

let touchStartX = 0;
let touchStartY = 0;
let touchStartTime = 0;
let lastTouchX = 0;
let lastTouchTime = 0;
let touchVelocityX = 0; // px / ms
let isHorizontalGesture: boolean | null = null;
let activeRafId: number | null = null;
const mobileSettleDuration = ref(320);

const mobileTrackStyle = computed(() => {
  const baseOffset = workspaceStore.activePanelId === 'left' ? 0 : -50;

  if (isDraggingMobileTrack.value) {
    // When actively dragging, translate continuously with live delta
    return {
      transform: `translate3d(calc(${baseOffset}% + ${dragDeltaX.value}px), 0, 0)`,
      transition: 'none'
    };
  }

  // When settling or toggled via button, apply Apple spring curve with velocity-dynamic duration
  return {
    transform: `translate3d(${baseOffset}%, 0, 0)`,
    transition: `transform ${mobileSettleDuration.value}ms cubic-bezier(0.32, 0.72, 0, 1)`
  };
});

function handleTouchStart(e: TouchEvent) {
  if (!uiStore.isMobile || !workspaceStore.isDualPane || e.touches.length === 0) return;
  const touch = e.touches[0];
  touchStartX = touch.clientX;
  touchStartY = touch.clientY;
  touchStartTime = performance.now();
  lastTouchX = touch.clientX;
  lastTouchTime = touchStartTime;
  touchVelocityX = 0;
  isHorizontalGesture = null;
  dragDeltaX.value = 0;
  mobileSettleDuration.value = 320;
}

function handleTouchMove(e: TouchEvent) {
  if (!uiStore.isMobile || !workspaceStore.isDualPane || e.touches.length === 0) return;
  const touch = e.touches[0];
  const dx = touch.clientX - touchStartX;
  const dy = touch.clientY - touchStartY;

  // Determine gesture direction on initial movement
  if (isHorizontalGesture === null) {
    if (Math.abs(dx) > 6 || Math.abs(dy) > 6) {
      isHorizontalGesture = Math.abs(dx) > Math.abs(dy) * 1.15;
    }
  }

  if (isHorizontalGesture) {
    e.preventDefault(); // Lock vertical scroll during horizontal panel swipe
    isDraggingMobileTrack.value = true;

    const now = performance.now();
    const dt = now - lastTouchTime;
    if (dt > 10) {
      touchVelocityX = (touch.clientX - lastTouchX) / dt;
      lastTouchX = touch.clientX;
      lastTouchTime = now;
    }

    // Apply Apple-style Rubber-Banding Resistance when overscrolling boundaries
    let effectiveDx = dx;
    if (workspaceStore.activePanelId === 'left' && dx > 0) {
      effectiveDx = dx * 0.3;
    } else if (workspaceStore.activePanelId === 'right' && dx < 0) {
      effectiveDx = dx * 0.3;
    }

    if (activeRafId) cancelAnimationFrame(activeRafId);
    activeRafId = requestAnimationFrame(() => {
      dragDeltaX.value = effectiveDx;
    });
  }
}

function handleTouchEnd() {
  if (!uiStore.isMobile || !workspaceStore.isDualPane) return;
  if (activeRafId) cancelAnimationFrame(activeRafId);

  if (isDraggingMobileTrack.value) {
    isDraggingMobileTrack.value = false;
    const finalDeltaX = dragDeltaX.value;
    dragDeltaX.value = 0;

    // Calculate momentum-driven spring settle duration based on touch release velocity
    mobileSettleDuration.value = getDynamicSettleDuration(touchVelocityX, 320, 180);

    const containerWidth = mobileTrackWrapperRef.value?.clientWidth || window.innerWidth;
    const distanceThreshold = containerWidth * 0.26;
    const velocityThreshold = 0.4; // px / ms

    // Fast velocity flick or passed distance threshold
    if (workspaceStore.activePanelId === 'left') {
      if (finalDeltaX < -distanceThreshold || touchVelocityX < -velocityThreshold) {
        workspaceStore.setActivePanel('right');
      }
    } else if (workspaceStore.activePanelId === 'right') {
      if (finalDeltaX > distanceThreshold || touchVelocityX > velocityThreshold) {
        workspaceStore.setActivePanel('left');
      }
    }
  }

  isHorizontalGesture = null;
}

function handleTouchCancel() {
  if (isDraggingMobileTrack.value) {
    isDraggingMobileTrack.value = false;
    dragDeltaX.value = 0;
  }
  isHorizontalGesture = null;
}

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

// ==========================================================================
// Compositor-Synced Desktop Split Pane Resize (Direct CSS Var via rAF)
// ==========================================================================
function startSplitResize(e: MouseEvent) {
  e.preventDefault();
  isResizingSplit = true;
  currentPendingRatio = workspaceStore.splitRatio;
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
  window.addEventListener('mousemove', onSplitResizeMove, { passive: true });
  window.addEventListener('mouseup', stopSplitResize);
}

function onSplitResizeMove(e: MouseEvent) {
  if (!isResizingSplit || !mainContainerRef.value) return;
  const rect = mainContainerRef.value.getBoundingClientRect();
  const rawRatio = (e.clientX - rect.left) / rect.width;
  const ratio = Math.max(0.18, Math.min(0.82, rawRatio));
  currentPendingRatio = ratio;

  if (splitResizeRafId) cancelAnimationFrame(splitResizeRafId);
  splitResizeRafId = requestAnimationFrame(() => {
    if (mainContainerRef.value) {
      mainContainerRef.value.style.setProperty('--split-ratio', `${ratio}`);
    }
  });
}

function stopSplitResize() {
  if (isResizingSplit) {
    isResizingSplit = false;
    if (splitResizeRafId) cancelAnimationFrame(splitResizeRafId);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    window.removeEventListener('mousemove', onSplitResizeMove);
    window.removeEventListener('mouseup', stopSplitResize);
    // Commit to Pinia store once on release
    workspaceStore.setSplitRatio(currentPendingRatio);
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
