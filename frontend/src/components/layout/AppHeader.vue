<template>
  <header class="h-16 bg-white dark:bg-[#0b0f19] border-b border-gray-200 dark:border-slate-800/80 px-6 flex items-center justify-between text-gray-800 dark:text-slate-100 select-none sticky top-0 z-20 overflow-visible">
    <!-- Left: Breadcrumbs Bar (Clean, Unclipped Dropdown) -->
    <div class="flex items-center space-x-1 max-w-[55vw] text-base font-medium shrink min-w-0 py-1">
      <!-- Clean Root Breadcrumb + Dropdown Trigger -->
      <div class="relative shrink-0 flex items-center">
        <!-- Root Navigation Button -->
        <button
          @click="workspaceStore.navigatePanel(workspaceStore.activePanelId, '/')"
          :class="[
            'flex items-center space-x-1.5 px-2 py-1 rounded-lg transition truncate cursor-pointer',
            activePanel.path === '/'
              ? 'text-gray-950 dark:text-white font-bold cursor-default'
              : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800'
          ]"
          :title="`Go to ${activeSourceName} (/)`"
        >
          <FbIcon
            :name="activeConnection?.provider === 'local' ? 'folder' : 'share'"
            size="16px"
            :class="activePanel.path === '/' ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400 dark:text-slate-500'"
          />
          <span class="truncate">{{ activeSourceName }}</span>
        </button>

        <!-- Dropdown Chevron Button -->
        <button
          @click.stop="isSourceMenuOpen = !isSourceMenuOpen"
          :class="[
            'p-1 text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-md transition cursor-pointer flex items-center justify-center',
            isSourceMenuOpen ? 'text-blue-600 dark:text-blue-400 bg-gray-100 dark:bg-slate-800' : ''
          ]"
          title="Switch Storage Source"
        >
          <FbIcon
            name="chevron-down"
            size="12px"
            :class="['transition-transform duration-200', isSourceMenuOpen ? 'rotate-180 text-blue-600 dark:text-blue-400' : '']"
          />
        </button>

        <!-- Source Dropdown Popover Menu -->
        <div
          v-if="isSourceMenuOpen"
          @click="isSourceMenuOpen = false"
          class="absolute left-0 top-full mt-2 w-64 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-2xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5 animate-in fade-in zoom-in-95 duration-100"
        >
          <div class="px-3 py-1.5 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
            STORAGE SOURCES
          </div>
          <button
            v-for="conn in connStore.connections"
            :key="conn.id"
            @click="handleSelectSource(conn.id)"
            :class="[
              'w-full flex items-center justify-between px-3 py-2.5 rounded-xl transition text-left cursor-pointer group',
              activePanel.connectionId === conn.id
                ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-semibold ring-1 ring-blue-500/20'
                : 'hover:bg-gray-100 dark:hover:bg-slate-800/80 text-gray-700 dark:text-slate-300'
            ]"
          >
            <div class="flex items-center space-x-2.5 truncate">
              <FbIcon
                :name="conn.provider === 'local' ? 'folder' : 'share'"
                size="16px"
                :class="activePanel.connectionId === conn.id ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400 group-hover:text-gray-600 dark:group-hover:text-slate-200'"
              />
              <span class="truncate font-medium">{{ conn.name }}</span>
            </div>
            <div class="flex items-center space-x-1.5 shrink-0 ml-2">
              <span
                v-if="conn.provider !== 'local'"
                class="text-[9px] uppercase font-mono px-1.5 py-0.5 rounded bg-gray-200/70 dark:bg-slate-800 text-gray-500 dark:text-slate-400"
              >
                {{ conn.provider }}
              </span>
              <span v-if="activePanel.connectionId === conn.id" class="text-blue-600 dark:text-blue-400 text-xs font-bold">✓</span>
            </div>
          </button>
          <div class="my-1 border-t border-gray-100 dark:border-slate-800/80"></div>
          <button
            @click="emit('openConnectionDialog')"
            class="w-full flex items-center space-x-2 px-3 py-2 rounded-xl hover:bg-blue-50 dark:hover:bg-blue-950/40 text-blue-600 dark:text-blue-400 font-semibold transition text-left cursor-pointer"
          >
            <FbIcon name="plus" size="14px" />
            <span>Add Storage Connection...</span>
          </button>
        </div>
      </div>

      <!-- Breadcrumbs Segments -->
      <template v-for="(crumb, idx) in breadcrumbs" :key="crumb.path">
        <span class="text-gray-400 dark:text-slate-600 px-0.5 shrink-0">
          <FbIcon name="chevron-right" size="14px" />
        </span>
        <button
          @click="workspaceStore.navigatePanel(workspaceStore.activePanelId, crumb.path)"
          :class="[
            'px-1.5 py-1 rounded-lg transition truncate shrink min-w-0 max-w-[140px] sm:max-w-[200px]',
            idx === breadcrumbs.length - 1
              ? 'text-gray-950 dark:text-white font-bold cursor-default pointer-events-none'
              : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 cursor-pointer'
          ]"
          :title="crumb.name"
        >
          {{ crumb.name }}
        </button>
      </template>
    </div>

    <!-- Right: Toolbar Controls (Search, View Mode, Sort & Filter, Select, Dual Pane, + New) -->
    <div class="flex items-center space-x-2 shrink-0">
      <!-- Search Input Box (Pill shape with ⌘K badge) -->
      <button
        @click="emit('openSearchDialog')"
        class="bg-white dark:bg-slate-900/90 hover:bg-gray-50 dark:hover:bg-slate-800/80 border border-gray-200 dark:border-slate-800 px-3.5 py-2 rounded-xl text-gray-400 hover:text-gray-600 dark:hover:text-slate-300 flex items-center space-x-2.5 text-xs transition shadow-xs w-48 sm:w-56 justify-between cursor-pointer"
        title="Search files (Ctrl+K / Cmd+K)"
      >
        <div class="flex items-center space-x-2">
          <FbIcon name="search" size="16px" class="text-gray-400" />
          <span class="text-gray-400 font-normal">Search...</span>
        </div>
        <kbd class="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-slate-800 text-[10px] text-gray-500 dark:text-slate-400 border border-gray-200 dark:border-slate-700 font-mono">⌘K</kbd>
      </button>

      <!-- View Switcher & Action Group -->
      <div class="flex items-center border border-gray-200 dark:border-slate-800 rounded-xl p-0.5 bg-gray-50/50 dark:bg-slate-900">
        <!-- List View Button -->
        <button
          @click="activePanel.viewMode = 'list'; workspaceStore.saveState()"
          :class="[
            'p-2 rounded-lg transition text-gray-600 dark:text-slate-400 cursor-pointer',
            activePanel.viewMode === 'list'
              ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-xs'
              : 'hover:bg-gray-100 dark:hover:bg-slate-800/50'
          ]"
          title="List View"
        >
          <FbIcon name="list" size="16px" />
        </button>

        <!-- Grid View Button -->
        <button
          @click="activePanel.viewMode = 'grid'; workspaceStore.saveState()"
          :class="[
            'p-2 rounded-lg transition text-gray-600 dark:text-slate-400 cursor-pointer',
            activePanel.viewMode === 'grid'
              ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-xs'
              : 'hover:bg-gray-100 dark:hover:bg-slate-800/50'
          ]"
          title="Grid / Mosaic View"
        >
          <FbIcon name="grid" size="16px" />
        </button>
      </div>

      <!-- Interactive Sort & Filter Dropdown Popover -->
      <div class="relative">
        <button
          @click="isSortMenuOpen = !isSortMenuOpen"
          :class="[
            'p-2 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 transition cursor-pointer flex items-center space-x-1',
            isSortMenuOpen || (activePanel.filterType && activePanel.filterType !== 'all') || activePanel.sortField !== 'name'
              ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
              : 'text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
          ]"
          :title="`Sort & Filter (${activePanel.sortField}, ${activePanel.sortOrder})`"
        >
          <FbIcon name="sort" size="18px" />
          <span
            v-if="activePanel.filterType && activePanel.filterType !== 'all'"
            class="w-2 h-2 rounded-full bg-blue-600 dark:bg-blue-400 animate-pulse"
          ></span>
        </button>

        <!-- Sort & Filter Popover Menu -->
        <div
          v-if="isSortMenuOpen"
          @click.stop
          class="absolute right-0 mt-2 w-60 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-2xl p-2.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-2.5 animate-in fade-in zoom-in-95 duration-100"
        >
          <!-- SECTION 1: SORT BY -->
          <div>
            <div class="px-2 py-1 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
              SORT BY
            </div>
            <div class="space-y-0.5">
              <button
                v-for="field in sortFields"
                :key="field.id"
                @click="setSortField(field.id)"
                :class="[
                  'w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl transition text-left cursor-pointer',
                  activePanel.sortField === field.id
                    ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-semibold'
                    : 'hover:bg-gray-100 dark:hover:bg-slate-800/80 text-gray-700 dark:text-slate-300'
                ]"
              >
                <span>{{ field.label }}</span>
                <span v-if="activePanel.sortField === field.id" class="text-blue-600 dark:text-blue-400 font-bold">✓</span>
              </button>
            </div>
          </div>

          <div class="border-t border-gray-100 dark:border-slate-800/80"></div>

          <!-- SECTION 2: ORDER (ASC / DESC) -->
          <div>
            <div class="px-2 py-1 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
              ORDER
            </div>
            <div class="grid grid-cols-2 gap-1 p-0.5 bg-gray-100 dark:bg-slate-800/80 rounded-xl">
              <button
                @click="setSortOrder('asc')"
                :class="[
                  'py-1.5 text-center rounded-lg font-medium transition cursor-pointer text-xs',
                  activePanel.sortOrder === 'asc'
                    ? 'bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-xs font-semibold'
                    : 'text-gray-500 hover:text-gray-900 dark:text-slate-400'
                ]"
              >
                Ascending ↑
              </button>
              <button
                @click="setSortOrder('desc')"
                :class="[
                  'py-1.5 text-center rounded-lg font-medium transition cursor-pointer text-xs',
                  activePanel.sortOrder === 'desc'
                    ? 'bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-xs font-semibold'
                    : 'text-gray-500 hover:text-gray-900 dark:text-slate-400'
                ]"
              >
                Descending ↓
              </button>
            </div>
          </div>

          <div class="border-t border-gray-100 dark:border-slate-800/80"></div>

          <!-- SECTION 3: QUICK FILTER TYPE -->
          <div>
            <div class="px-2 py-1 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider flex items-center justify-between">
              <span>FILTER CATEGORY</span>
              <button
                v-if="activePanel.filterType && activePanel.filterType !== 'all'"
                @click="setFilterType('all')"
                class="text-blue-600 dark:text-blue-400 hover:underline cursor-pointer lowercase text-[10px] font-semibold"
              >
                Reset All
              </button>
            </div>
            <div class="space-y-0.5 max-h-40 overflow-y-auto">
              <button
                v-for="flt in filterOptions"
                :key="flt.id"
                @click="setFilterType(flt.id)"
                :class="[
                  'w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl transition text-left cursor-pointer',
                  (activePanel.filterType || 'all') === flt.id
                    ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-semibold'
                    : 'hover:bg-gray-100 dark:hover:bg-slate-800/80 text-gray-700 dark:text-slate-300'
                ]"
              >
                <div class="flex items-center space-x-2">
                  <span class="text-sm">{{ flt.icon }}</span>
                  <span>{{ flt.label }}</span>
                </div>
                <span v-if="(activePanel.filterType || 'all') === flt.id" class="text-blue-600 dark:text-blue-400 font-bold">✓</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Select Mode / Toggle All Button -->
      <button
        @click="toggleSelectAll"
        :class="[
          'p-2 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 transition cursor-pointer',
          activePanel.selectedEntries.length > 0
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50'
            : 'text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
        ]"
        title="Select Mode"
      >
        <FbIcon name="select-all" size="18px" />
      </button>

      <!-- Show/Hide Hidden Files (Dotfiles) Toggle Button -->
      <button
        @click="toggleHidden"
        :class="[
          'p-2 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 transition cursor-pointer',
          activePanel.showHidden
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50'
            : 'text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
        ]"
        :title="activePanel.showHidden ? 'Hide Dotfiles / Hidden Items (Ctrl+H)' : 'Show Dotfiles / Hidden Items (Ctrl+H)'"
      >
        <FbIcon name="eye" size="18px" />
      </button>

      <!-- Dual Pane Switcher Button (Persistent) -->
      <button
        @click="workspaceStore.setDualPane(!workspaceStore.isDualPane)"
        :class="[
          'p-2 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 transition cursor-pointer',
          workspaceStore.isDualPane
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50'
            : 'text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
        ]"
        title="Toggle Dual Pane View"
      >
        <FbIcon name="panel-right" size="18px" />
      </button>

      <!-- Primary Action Button: + New Dropdown -->
      <div class="relative">
        <button
          @click="isNewMenuOpen = !isNewMenuOpen"
          class="bg-blue-600 hover:bg-blue-700 active:bg-blue-800 text-white font-semibold px-4 py-2 rounded-xl flex items-center space-x-1.5 text-sm shadow-xs transition cursor-pointer"
        >
          <FbIcon name="plus" size="16px" />
          <span>New</span>
        </button>

        <!-- Dropdown Menu -->
        <div
          v-if="isNewMenuOpen"
          @click="isNewMenuOpen = false"
          class="absolute right-0 mt-2 w-44 bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5 animate-in fade-in zoom-in-95 duration-100"
        >
          <button
            @click="openNew('file')"
            class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
          >
            <FbIcon name="new-file" size="16px" class="text-blue-600" />
            <span>New File</span>
          </button>
          <button
            @click="openNew('directory')"
            class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
          >
            <FbIcon name="new-folder" size="16px" class="text-amber-500" />
            <span>New Folder</span>
          </button>
          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
          <button
            @click="openUpload"
            class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
          >
            <FbIcon name="upload" size="16px" class="text-emerald-500" />
            <span>Upload Files</span>
          </button>
        </div>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useConnectionStore } from '../../stores/connectionStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';

const emit = defineEmits<{
  (e: 'openSearchDialog'): void;
  (e: 'openConnectionDialog'): void;
  (e: 'openAuditLogDialog'): void;
}>();

const connStore = useConnectionStore();
const workspaceStore = useWorkspaceStore();
const fileStore = useFileStore();
const uiStore = useUiStore();

const isNewMenuOpen = ref(false);
const isSourceMenuOpen = ref(false);
const isSortMenuOpen = ref(false);

const sortFields = [
  { id: 'name', label: 'Name' },
  { id: 'size', label: 'File Size' },
  { id: 'modified', label: 'Modified Date' },
  { id: 'type', label: 'File Type / Ext' },
];

const filterOptions = [
  { id: 'all', label: 'All Items', icon: '📁' },
  { id: 'folders', label: 'Folders Only', icon: '📂' },
  { id: 'images', label: 'Images / Photos', icon: '🖼️' },
  { id: 'videos', label: 'Videos & Movies', icon: '🎬' },
  { id: 'audio', label: 'Music & Audio', icon: '🎵' },
  { id: 'code', label: 'Documents & Code', icon: '📝' },
  { id: 'archives', label: 'Archives (Zip/Tar)', icon: '📦' },
];

const activePanel = computed(() => workspaceStore.getPanel(workspaceStore.activePanelId));

const activeConnection = computed(() => {
  return connStore.connections.find((c) => c.id === activePanel.value.connectionId);
});

const activeSourceName = computed(() => {
  return activeConnection.value ? activeConnection.value.name : 'Local Storage';
});

const breadcrumbs = computed(() => {
  const parts = activePanel.value.path.split('/').filter(Boolean);
  const crumbs: { name: string; path: string }[] = [];
  let current = '';

  for (const part of parts) {
    current += `/${part}`;
    crumbs.push({ name: part, path: current });
  }

  // Handle truncation with ... if more than 3 segments
  if (crumbs.length > 3) {
    const lastThree = crumbs.slice(-3);
    return [
      { name: '...', path: crumbs[crumbs.length - 4].path },
      ...lastThree,
    ];
  }

  return crumbs;
});

function handleSelectSource(connId: string) {
  fileStore.currentConnectionId = connId;
  workspaceStore.switchPanelConnection(workspaceStore.activePanelId, connId, '/');
  isSourceMenuOpen.value = false;
}

function setSortField(field: string) {
  activePanel.value.sortField = field;
}

function setSortOrder(order: 'asc' | 'desc') {
  activePanel.value.sortOrder = order;
}

function setFilterType(type: string) {
  activePanel.value.filterType = type;
  isSortMenuOpen.value = false;
}

function toggleSelectAll() {
  if (activePanel.value.selectedEntries.length === activePanel.value.entries.length) {
    activePanel.value.selectedEntries = [];
  } else {
    activePanel.value.selectedEntries = activePanel.value.entries.map((e) => e.path);
  }
}

async function toggleHidden() {
  await workspaceStore.toggleShowHidden(workspaceStore.activePanelId);
  uiStore.showToast(
    activePanel.value.showHidden ? 'Showing dotfiles & hidden items' : 'Hiding dotfiles',
    'info'
  );
}

function openNew(type: 'file' | 'directory') {
  fileStore.currentConnectionId = activePanel.value.connectionId;
  fileStore.currentPath = activePanel.value.path;
  uiStore.openCreate(type);
}

function openUpload() {
  fileStore.currentConnectionId = activePanel.value.connectionId;
  fileStore.currentPath = activePanel.value.path;
  uiStore.openUpload();
}

function handleOutsideClick(e: MouseEvent) {
  const target = e.target as HTMLElement;
  if (isNewMenuOpen.value && !target.closest('.relative')) {
    isNewMenuOpen.value = false;
  }
  if (isSourceMenuOpen.value && !target.closest('.relative')) {
    isSourceMenuOpen.value = false;
  }
  if (isSortMenuOpen.value && !target.closest('.relative')) {
    isSortMenuOpen.value = false;
  }
}

onMounted(() => {
  window.addEventListener('click', handleOutsideClick);
});

onUnmounted(() => {
  window.removeEventListener('click', handleOutsideClick);
});
</script>
