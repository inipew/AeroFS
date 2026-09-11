<template>
  <div class="h-full w-full flex flex-col min-h-0 overflow-hidden relative">
    <!-- Universal App TopBar (Global Chrome Only) -->
    <AppTopBar />

    <!-- Workspace Body (Sidebar + Content Workspace Area) -->
    <div class="flex-1 flex min-h-0 overflow-hidden relative">
      <!-- Collapsible / Floating Navigation Sidebar -->
      <AppSidebar
        @open-connection-dialog="overlayStore.open({ type: 'connection' })"
        @open-edit-connection-dialog="handleOpenEditConnection"
        @open-delete-connection-dialog="handleOpenDeleteConnection"
        @open-settings-dialog="overlayStore.open({ type: 'settings' })"
        @open-shares-dialog="overlayStore.open({ type: 'shares' })"
        @open-trash-dialog="overlayStore.open({ type: 'trash' })"
        @open-starred-dialog="overlayStore.open({ type: 'starred' })"
        @open-starred-view="overlayStore.open({ type: 'starred' })"
        @open-recent-dialog="overlayStore.open({ type: 'recent' })"
        @open-recent-view="overlayStore.open({ type: 'recent' })"
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

        <!-- Dynamic Workspace Viewport Presenter -->
        <WorkspaceViewport />

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
            @click="overlayStore.open({ type: 'connection' })"
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
            @click="overlayStore.open({ type: 'settings' })"
            class="flex-1 flex flex-col items-center justify-center py-1 space-y-1 hover:text-blue-600 dark:hover:text-blue-400 transition-colors duration-fast cursor-pointer min-h-[44px]"
          >
            <FbIcon name="settings" size="18px" />
            <span>Settings</span>
          </button>
        </nav>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import FbIcon from '../components/common/FbIcon.vue';
import AppTopBar from './AppTopBar.vue';
import AppSidebar from '../components/layout/AppSidebar.vue';
import WorkspaceViewport from '../workspace/WorkspaceViewport.vue';
import { useWorkspaceStore } from '../stores/workspaceStore';
import { useConnectionStore } from '../stores/connectionStore';
import { useUiStore } from '../stores/uiStore';
import { useTransferStore } from '../stores/transferStore';
import { useOverlayStore } from '../overlays/overlayStore';
import type { Connection } from '../types/connection';

const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();
const uiStore = useUiStore();
const transferStore = useTransferStore();
const overlayStore = useOverlayStore();

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

function handleOpenEditConnection(conn: Connection) {
  overlayStore.open({ type: 'connection', connectionToEdit: conn });
}

function handleOpenDeleteConnection(conn: Connection) {
  overlayStore.open({ type: 'delete-connection', connection: conn });
}
</script>
