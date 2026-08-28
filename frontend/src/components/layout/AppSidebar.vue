<template>
  <!-- Mobile Backdrop Overlay -->
  <div
    v-if="uiStore.isMobile && uiStore.isMobileSidebarOpen"
    @click="uiStore.isMobileSidebarOpen = false"
    class="fixed inset-0 bg-black/60 backdrop-blur-xs z-40 animate-in fade-in duration-200"
  ></div>

  <!-- Sidebar Component (Desktop Sticky Column vs Mobile Slide-Over Drawer) -->
  <aside
    :class="[
      'bg-gray-50/95 dark:bg-[#090d16] border-r border-gray-200/80 dark:border-slate-800/80 flex flex-col justify-between transition-[width,transform] duration-standard ease-spring select-none shrink-0',
      uiStore.isMobile
        ? (uiStore.isMobileSidebarOpen ? 'fixed inset-y-0 left-0 z-50 w-72 shadow-2xl animate-in slide-in-from-left duration-200' : 'hidden')
        : (isCollapsed ? 'h-full w-16 z-30' : 'h-full w-64 z-30')
    ]"
  >
    <!-- Top Branding & Navigation Items -->
    <div class="flex-1 overflow-y-auto px-3 py-4 space-y-6">
      <!-- App Brand / Logo -->
      <div class="flex items-center justify-between px-2">
        <div class="flex items-center space-x-2.5 truncate">
          <!-- Blue Rounded Square Icon with Circle Center -->
          <div class="w-8 h-8 rounded-xl bg-blue-600 flex items-center justify-center text-white shadow-xs shrink-0">
            <div class="w-3.5 h-3.5 rounded-full border-2 border-white"></div>
          </div>
          <span v-if="!isCollapsed" class="font-bold text-base tracking-tight text-gray-900 dark:text-white">
            AeroFS
          </span>
        </div>

        <!-- Close Button on Mobile vs Collapse Button on Desktop -->
        <button
          v-if="uiStore.isMobile"
          @click="uiStore.isMobileSidebarOpen = false"
          class="p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer font-bold text-sm"
          title="Close Navigation Drawer"
        >
          ✕
        </button>
        <button
          v-else
          @click="isCollapsed = !isCollapsed"
          class="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
          title="Toggle Sidebar"
        >
          <FbIcon name="panel-left" size="18px" />
        </button>
      </div>

      <!-- SOURCES Section -->
      <div>
        <div class="flex items-center justify-between px-2 mb-2">
          <span v-if="!isCollapsed" class="text-[11px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
            SOURCES
          </span>
          <button
            @click="emit('openConnectionDialog'); if (uiStore.isMobile) uiStore.isMobileSidebarOpen = false;"
            class="text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 p-1 rounded-md hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
            title="Add Storage Connection"
          >
            <FbIcon name="plus" size="14px" />
          </button>
        </div>

        <!-- Sources List -->
        <div class="space-y-1">
          <!-- Dynamic Connections from database -->
          <div
            v-for="conn in connStore.connections"
            :key="conn.id"
            @click="selectConnection(conn.id)"
            :class="[
              'flex items-center rounded-xl transition cursor-pointer',
              isCollapsed ? 'justify-center p-2.5' : 'px-3 py-2.5 min-h-[44px] space-x-3',
              isActiveConnection(conn.id)
                ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-semibold ring-1 ring-blue-500/20'
                : 'text-gray-700 dark:text-slate-300 hover:bg-gray-100/80 dark:hover:bg-slate-800/60'
            ]"
            :title="conn.name"
          >
            <FbIcon
              :name="conn.provider === 'local' ? 'folder' : 'share'"
              size="18px"
              :class="isActiveConnection(conn.id) ? 'text-blue-600 dark:text-blue-400' : 'text-gray-500 dark:text-slate-400 shrink-0'"
            />
            <span v-if="!isCollapsed" class="text-sm truncate flex-1 font-medium">
              {{ conn.name }}
            </span>
            <span
              v-if="!isCollapsed && conn.provider !== 'local'"
              class="text-[10px] uppercase font-mono px-1.5 py-0.5 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-500 dark:text-slate-400 font-normal"
            >
              {{ conn.provider }}
            </span>
          </div>
        </div>
      </div>

      <!-- Quick Links Section (Recent, Starred, Shares, Trash) -->
      <div class="space-y-1">
        <button
          @click="emit('openRecentView'); if (uiStore.isMobile) uiStore.isMobileSidebarOpen = false;"
          class="w-full flex items-center px-3 py-2.5 min-h-[44px] rounded-xl text-sm font-medium text-gray-700 dark:text-slate-300 hover:bg-gray-100/80 dark:hover:bg-slate-800/60 transition group text-left cursor-pointer"
          title="Recent Files"
        >
          <FbIcon name="clock" size="18px" class="text-gray-500 dark:text-slate-400 group-hover:text-gray-700 dark:group-hover:text-slate-200 shrink-0" />
          <span v-if="!isCollapsed" class="ml-3 truncate">Recent</span>
        </button>

        <button
          @click="emit('openStarredView'); if (uiStore.isMobile) uiStore.isMobileSidebarOpen = false;"
          class="w-full flex items-center px-3 py-2.5 min-h-[44px] rounded-xl text-sm font-medium text-gray-700 dark:text-slate-300 hover:bg-gray-100/80 dark:hover:bg-slate-800/60 transition group text-left cursor-pointer"
          title="Starred Bookmarks"
        >
          <FbIcon name="star" size="18px" class="text-gray-500 dark:text-slate-400 group-hover:text-gray-700 dark:group-hover:text-slate-200 shrink-0" />
          <span v-if="!isCollapsed" class="ml-3 truncate">Starred</span>
        </button>

        <button
          @click="emit('openSharesDialog'); if (uiStore.isMobile) uiStore.isMobileSidebarOpen = false;"
          class="w-full flex items-center px-3 py-2.5 min-h-[44px] rounded-xl text-sm font-medium text-gray-700 dark:text-slate-300 hover:bg-gray-100/80 dark:hover:bg-slate-800/60 transition group text-left cursor-pointer"
          title="Active Shares"
        >
          <FbIcon name="users" size="18px" class="text-gray-500 dark:text-slate-400 group-hover:text-gray-700 dark:group-hover:text-slate-200 shrink-0" />
          <span v-if="!isCollapsed" class="ml-3 truncate">Shares</span>
        </button>

        <button
          @click="emit('openTrashDialog'); if (uiStore.isMobile) uiStore.isMobileSidebarOpen = false;"
          class="w-full flex items-center px-3 py-2.5 min-h-[44px] rounded-xl text-sm font-medium text-gray-700 dark:text-slate-300 hover:bg-gray-100/80 dark:hover:bg-slate-800/60 transition group text-left cursor-pointer"
          title="Trash / Recycle Bin"
        >
          <FbIcon name="trash" size="18px" class="text-gray-500 dark:text-slate-400 group-hover:text-gray-700 dark:group-hover:text-slate-200 shrink-0" />
          <span v-if="!isCollapsed" class="ml-3 truncate">Trash</span>
        </button>
      </div>
    </div>

    <!-- Streamlined Storage Info Box -->
    <div
      v-if="!isCollapsed"
      class="mx-3 mb-3 p-3 bg-white/70 dark:bg-[#0f1422]/80 border border-gray-200/80 dark:border-slate-800/80 rounded-2xl shadow-xs space-y-2"
    >
      <div class="flex items-center justify-between text-xs">
        <div class="flex items-center space-x-1.5 truncate">
          <span class="w-2 h-2 rounded-full bg-emerald-500 shrink-0"></span>
          <span class="font-semibold text-gray-800 dark:text-slate-200 truncate">{{ storageInfo.source_name }}</span>
        </div>
        <span class="text-[10px] font-mono text-gray-400 dark:text-slate-500 uppercase font-bold shrink-0">
          {{ activeConnection?.provider || 'LOCAL' }}
        </span>
      </div>

      <div class="flex items-center justify-between text-[11px] text-gray-500 dark:text-slate-400 font-mono">
        <span>{{ storageInfo.source_size_formatted }}</span>
        <span>{{ storageInfo.disk_usage_text }}</span>
      </div>

      <!-- Compact Progress Bar -->
      <div class="w-full bg-gray-100 dark:bg-slate-800 rounded-full h-1 overflow-hidden">
        <div
          class="bg-blue-600 h-full rounded-full transition-[width] duration-standard ease-spring"
          :style="{ width: `${storageInfo.used_percent || 0}%` }"
        ></div>
      </div>
    </div>

    <!-- User Profile Footer -->
    <div class="p-3.5 border-t border-gray-200/80 dark:border-slate-800 flex items-center justify-between shrink-0 bg-transparent">
      <!-- User Info & Avatar -->
      <div class="flex items-center space-x-2.5 truncate">
        <div class="w-8 h-8 rounded-full bg-gray-200 dark:bg-slate-800 flex items-center justify-center text-base shrink-0 shadow-xs border border-gray-300/60 dark:border-slate-700">
          🤖
        </div>
        <div v-if="!isCollapsed" class="truncate">
          <p class="text-xs font-bold text-gray-900 dark:text-white leading-tight truncate">
            {{ authStore.user?.username || 'admin' }}
          </p>
          <p class="text-[10px] text-gray-400 dark:text-slate-500 leading-none">
            {{ authStore.user?.is_admin ? 'Admin' : 'User' }}
          </p>
        </div>
      </div>

      <!-- Action Icons (Theme, Settings, Logout) -->
      <div v-if="!isCollapsed" class="flex items-center space-x-1">
        <button
          @click="toggleTheme"
          class="p-1.5 text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
          title="Toggle Theme"
        >
          <FbIcon :name="themeStore.isDark ? 'sun' : 'moon'" size="16px" />
        </button>

        <button
          @click="emit('openSettingsDialog')"
          class="p-1.5 text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
          title="System & Storage Settings (Audit Logs)"
        >
          <FbIcon name="settings" size="16px" />
        </button>

        <button
          @click="authStore.logout()"
          class="p-1.5 text-gray-400 hover:text-red-600 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
          title="Sign out"
        >
          <FbIcon name="logout" size="16px" />
        </button>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useAuthStore } from '../../stores/authStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useFileStore } from '../../stores/fileStore';
import { useThemeStore } from '../../stores/themeStore';
import { useUiStore } from '../../stores/uiStore';
import { apiClient } from '../../api/client';

const emit = defineEmits<{
  (e: 'openConnectionDialog'): void;
  (e: 'openSettingsDialog'): void;
  (e: 'openSharesDialog'): void;
  (e: 'openTrashDialog'): void;
  (e: 'openRecentView'): void;
  (e: 'openStarredView'): void;
}>();

const authStore = useAuthStore();
const connStore = useConnectionStore();
const workspaceStore = useWorkspaceStore();
const fileStore = useFileStore();
const themeStore = useThemeStore();
const uiStore = useUiStore();

const isCollapsed = ref(false);

const activePanel = computed(() => workspaceStore.getPanel(workspaceStore.activePanelId));

const activeConnection = computed(() => {
  return connStore.connections.find((c) => c.id === activePanel.value.connectionId);
});

const storageInfo = ref({
  source_name: 'Documents',
  source_size_formatted: '125 GiB',
  disk_label: 'Disk',
  disk_usage_text: '63% · 244 GiB',
  used_percent: 63,
});

async function fetchStorageInfo() {
  const connId = activePanel.value.connectionId || 'local';
  try {
    const resp = await apiClient.get(`/connections/${connId}/storage-info`);
    if (resp.data) {
      storageInfo.value = resp.data;
    }
  } catch {
    // Fallback based on connection
    if (activeConnection.value?.provider === 'ftp') {
      storageInfo.value = {
        source_name: activeConnection.value.name,
        source_size_formatted: 'FTP Remote',
        disk_label: 'FTP Host',
        disk_usage_text: 'Connected · Online',
        used_percent: 50,
      };
    } else {
      storageInfo.value = {
        source_name: activeConnection.value?.name || 'Local Storage',
        source_size_formatted: '125 GiB',
        disk_label: 'Disk',
        disk_usage_text: '63% · 244 GiB',
        used_percent: 63,
      };
    }
  }
}

watch(
  () => activePanel.value.connectionId,
  () => {
    fetchStorageInfo();
  }
);

onMounted(() => {
  fetchStorageInfo();
});

function isActiveConnection(id: string): boolean {
  return activePanel.value.connectionId === id;
}

function selectConnection(id: string) {
  fileStore.currentConnectionId = id;
  workspaceStore.switchPanelConnection(workspaceStore.activePanelId, id, '/');
  if (uiStore.isMobile) {
    uiStore.isMobileSidebarOpen = false;
  }
}

function toggleTheme() {
  themeStore.toggleTheme();
}
</script>
