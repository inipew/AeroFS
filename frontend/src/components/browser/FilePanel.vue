<template>
  <div
    @click="workspaceStore.setActivePanel(panelId)"
    :class="[
      'flex-1 flex flex-col h-full bg-white dark:bg-[#0b0f19] overflow-hidden transition-all duration-150 relative select-none',
      workspaceStore.isDualPane
        ? (isActive
            ? 'ring-2 ring-blue-500/60 border border-blue-500/30 rounded-3xl shadow-xl'
            : 'border border-gray-200/90 dark:border-slate-800/90 rounded-3xl opacity-85 hover:opacity-100 hover:border-gray-300 dark:hover:border-slate-700 shadow-xs')
        : ''
    ]"
  >
    <!-- Dual-Pane Subheader with Connection Switcher & Close / Disconnect Button -->
    <div
      v-if="workspaceStore.isDualPane"
      :class="[
        'h-12 border-b px-4 flex items-center justify-between transition-colors text-xs shrink-0',
        isActive
          ? 'bg-blue-50/40 dark:bg-blue-950/20 border-blue-100 dark:border-blue-900/30'
          : 'bg-gray-50/60 dark:bg-slate-900/60 border-gray-200/80 dark:border-slate-800/80'
      ]"
    >
      <div class="flex items-center space-x-2 truncate">
        <!-- Connection Switcher Dropdown -->
        <select
          :value="panel.connectionId"
          @change="handleConnectionChange(($event.target as HTMLSelectElement).value)"
          class="bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-3 py-1.5 font-bold text-xs text-gray-800 dark:text-slate-100 cursor-pointer shadow-xs focus:outline-none focus:border-blue-500 transition"
        >
          <option v-for="conn in connStore.connections" :key="conn.id" :value="conn.id">
            {{ conn.name }} ({{ conn.provider.toUpperCase() }})
          </option>
        </select>

        <span class="text-gray-400 dark:text-slate-500 font-mono text-[11px] truncate max-w-[200px]">
          {{ panel.path }}
        </span>
      </div>

      <!-- Panel Action Buttons & Close Panel (✕) -->
      <div class="flex items-center space-x-1">
        <button
          @click.stop="workspaceStore.fetchPanelEntries(panelId)"
          class="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
          title="Reload panel"
        >
          <FbIcon name="refresh" size="14px" />
        </button>

        <button
          @click.stop="workspaceStore.closePanel(panelId)"
          class="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-950/40 transition cursor-pointer font-bold text-xs"
          title="Close / Disconnect Panel"
        >
          ✕
        </button>
      </div>
    </div>

    <!-- Drop Zone & File Listing Content -->
    <div
      @dragover.prevent="isDragOver = true"
      @dragleave.prevent="isDragOver = false"
      @drop.prevent="handleDrop"
      class="flex-1 overflow-y-auto p-6 relative"
    >
      <!-- Drop Overlay -->
      <div
        v-if="isDragOver"
        class="absolute inset-4 z-30 bg-blue-500/10 backdrop-blur-xs border-2 border-dashed border-blue-500 rounded-3xl flex items-center justify-center pointer-events-none"
      >
        <div class="bg-blue-600 text-white px-6 py-3 rounded-2xl shadow-2xl font-bold text-sm flex items-center space-x-2 animate-bounce">
          <FbIcon name="upload" size="20px" />
          <span>Drop files to copy into this folder</span>
        </div>
      </div>

      <!-- Empty State -->
      <div
        v-if="displayedFolders.length === 0 && displayedFiles.length === 0 && !panel.loading"
        class="py-24 flex flex-col items-center justify-center text-center"
      >
        <div class="w-16 h-16 rounded-2xl bg-gray-100 dark:bg-slate-800 flex items-center justify-center text-gray-400 mb-3">
          <FbIcon name="empty-folder" size="32px" />
        </div>
        <p class="font-semibold text-gray-800 dark:text-slate-200 text-base">This folder is empty</p>
        <p class="text-xs text-gray-400 dark:text-slate-500 mt-1">Upload files or create new folders to get started</p>
      </div>

      <!-- MOSAIC / GRID VIEW (Default matching screenshot) -->
      <div v-if="panel.viewMode === 'grid'" class="space-y-8">
        <!-- 1. FOLDERS SECTION -->
        <div v-if="displayedFolders.length > 0">
          <h2 class="text-xs font-bold uppercase tracking-wider text-gray-400 dark:text-slate-500 mb-3 px-1 flex items-center justify-between">
            <span>FOLDERS</span>
            <span class="text-[10px] font-mono text-gray-400 font-normal">({{ displayedFolders.length }})</span>
          </h2>

          <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-3">
            <div
              v-for="folder in displayedFolders"
              :key="folder.path"
              draggable="true"
              @dragstart="handleDragStart($event, folder)"
              @click="handleEntryClick($event, folder)"
              @dblclick="workspaceStore.navigatePanel(panelId, folder.path)"
              @contextmenu="openContextMenu($event, folder)"
              :class="[
                'border rounded-2xl px-4 py-3 flex items-center space-x-3 cursor-pointer transition-all duration-150 select-none shadow-xs group',
                isItemHidden(folder) ? 'opacity-65 hover:opacity-100 border-dashed border-gray-300 dark:border-slate-700 bg-gray-50/50 dark:bg-slate-900/40' : '',
                panel.selectedEntries.includes(folder.path)
                  ? 'bg-blue-50 dark:bg-blue-900/30 border-blue-500 ring-2 ring-blue-500/20'
                  : 'bg-white dark:bg-[#0f1422] border-gray-200/90 dark:border-slate-800/90 hover:border-blue-400 dark:hover:border-blue-500 hover:shadow-md'
              ]"
            >
              <!-- Folder Icon -->
              <FbIcon
                name="folder"
                size="20px"
                :class="[
                  'shrink-0 group-hover:scale-105 transition transform',
                  isItemHidden(folder) ? 'text-gray-400 dark:text-slate-500' : 'text-blue-600 dark:text-blue-400'
                ]"
              />
              <div class="truncate flex-1 flex items-center space-x-1.5 min-w-0">
                <span class="font-semibold text-xs text-gray-900 dark:text-white truncate">
                  {{ folder.name }}
                </span>
                <span v-if="isItemHidden(folder)" class="text-[9px] px-1 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-400 dark:text-slate-500 font-mono shrink-0">
                  dot
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- 2. FILES SECTION -->
        <div v-if="displayedFiles.length > 0">
          <h2 class="text-xs font-bold uppercase tracking-wider text-gray-400 dark:text-slate-500 mb-3 px-1 flex items-center justify-between">
            <span>FILES</span>
            <span class="text-[10px] font-mono text-gray-400 font-normal">({{ displayedFiles.length }})</span>
          </h2>

          <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4">
            <div
              v-for="file in displayedFiles"
              :key="file.path"
              draggable="true"
              @dragstart="handleDragStart($event, file)"
              @click="handleEntryClick($event, file)"
              @dblclick="handleEntryDoubleClick(file)"
              @contextmenu="openContextMenu($event, file)"
              :class="[
                'border rounded-2xl overflow-hidden cursor-pointer transition-all duration-200 flex flex-col group select-none shadow-xs',
                isItemHidden(file) ? 'opacity-65 hover:opacity-100 border-dashed border-gray-300 dark:border-slate-700 bg-gray-50/30 dark:bg-slate-900/30' : '',
                panel.selectedEntries.includes(file.path)
                  ? 'bg-blue-50 dark:bg-blue-900/30 border-blue-500 ring-2 ring-blue-500/20'
                  : 'bg-white dark:bg-[#0f1422] border-gray-200/90 dark:border-slate-800/90 hover:shadow-xl hover:-translate-y-1 hover:border-blue-400 dark:hover:border-blue-500'
              ]"
            >
              <!-- Card Thumbnail Area (Centered Absolute Overlays) -->
              <div
                class="h-36 sm:h-40 w-full bg-slate-100 dark:bg-slate-950/90 relative overflow-hidden shrink-0 border-b border-gray-100 dark:border-slate-800/80 flex items-center justify-center"
              >
                <!-- Real Image Preview -->
                <template v-if="isImage(file)">
                  <img
                    :src="getDownloadUrl(panel.connectionId, file.path)"
                    :alt="file.name"
                    class="w-full h-full object-cover group-hover:scale-105 transition duration-300"
                    loading="lazy"
                  />
                  <span class="absolute bottom-2 right-2 text-[9px] px-1.5 py-0.5 rounded-md bg-black/75 backdrop-blur-xs text-white/90 font-mono font-bold uppercase tracking-wider shadow-md border border-white/10">
                    {{ getFileExt(file) }}
                  </span>
                </template>

                <!-- Video Thumbnail with Real Snapshot Preview & Centered Play Overlay -->
                <template v-else-if="isVideo(file)">
                  <video
                    :src="getDownloadUrl(panel.connectionId, file.path) + '#t=0.5'"
                    preload="metadata"
                    muted
                    playsinline
                    class="w-full h-full object-cover group-hover:scale-105 transition duration-300 pointer-events-none"
                  ></video>
                  <div class="absolute inset-0 bg-gradient-to-t from-black/70 via-black/20 to-transparent group-hover:opacity-90 transition"></div>
                  <!-- Perfectly Centered Play Button -->
                  <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
                    <div class="w-11 h-11 rounded-full bg-black/50 backdrop-blur-md flex items-center justify-center text-white ring-1 ring-white/40 group-hover:scale-110 group-hover:bg-blue-600 transition duration-200 shadow-xl pl-0.5">
                      <FbIcon name="play" size="16px" class="fill-white" />
                    </div>
                  </div>
                  <span class="absolute bottom-2 right-2 text-[9px] px-1.5 py-0.5 rounded-md bg-black/75 backdrop-blur-xs text-white/90 font-mono font-bold uppercase tracking-wider shadow-md z-10 border border-white/10">
                    {{ getFileExt(file) }}
                  </span>
                </template>

                <!-- Audio Thumbnail with Music Visual Artwork -->
                <template v-else-if="isAudio(file)">
                  <div class="w-full h-full bg-gradient-to-br from-indigo-500/15 via-purple-500/15 to-pink-500/15 dark:from-indigo-950/50 dark:to-purple-950/50 flex flex-col items-center justify-center space-y-2">
                    <div class="w-12 h-12 rounded-2xl bg-gradient-to-tr from-blue-600 to-indigo-600 flex items-center justify-center text-white text-xl shadow-lg group-hover:scale-110 transition duration-200">
                      🎵
                    </div>
                    <span class="text-[10px] font-mono font-bold uppercase text-indigo-600 dark:text-indigo-400 tracking-wider">
                      {{ getFileExt(file) }}
                    </span>
                  </div>
                </template>

                <!-- Code / Script Preview (Adaptable for light and dark modes) -->
                <div v-else-if="isCode(file)" class="w-full h-full bg-slate-50 dark:bg-slate-900/90 p-3.5 text-[10px] font-mono text-slate-500 dark:text-slate-400 overflow-hidden flex flex-col justify-between">
                  <div class="space-y-1 opacity-70">
                    <div class="h-1.5 w-3/4 bg-blue-500/40 dark:bg-blue-500/40 rounded"></div>
                    <div class="h-1.5 w-1/2 bg-slate-300 dark:bg-slate-700 rounded"></div>
                    <div class="h-1.5 w-5/6 bg-slate-300 dark:bg-slate-700 rounded"></div>
                  </div>
                  <span class="text-blue-600 dark:text-blue-400 font-bold text-[11px] self-end uppercase">{{ getFileExt(file) }}</span>
                </div>

                <!-- Document / Archive / Other File Icon -->
                <div v-else class="flex flex-col items-center justify-center text-slate-400 dark:text-slate-500 space-y-1.5">
                  <FbIcon :name="getCategoryIcon(file)" size="34px" class="text-slate-400 dark:text-slate-500 group-hover:scale-110 transition transform duration-200" />
                  <span class="text-[9px] font-mono uppercase text-gray-500 dark:text-slate-400 font-bold tracking-wider">
                    {{ getFileExt(file) }}
                  </span>
                </div>
              </div>

              <!-- Card Bottom Footer -->
              <div class="p-3.5 bg-white dark:bg-[#0f1422] flex-1 flex flex-col justify-between">
                <div class="flex items-start justify-between gap-1.5">
                  <span class="font-bold text-xs text-gray-900 dark:text-white line-clamp-2 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition leading-snug" :title="file.name">
                    {{ file.name }}
                  </span>
                  <span v-if="isItemHidden(file)" class="text-[9px] px-1.5 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-500 dark:text-slate-400 font-mono shrink-0 mt-0.5">
                    dot
                  </span>
                </div>
                <span class="text-[11px] text-gray-400 dark:text-slate-500 mt-2 font-normal truncate">
                  {{ formatBytes(file.size || 0) }} · {{ formatRelativeTime(file.modified_at) }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- LIST TABLE VIEW -->
      <div v-else class="w-full">
        <table class="w-full text-left border-collapse text-xs select-none">
          <thead class="border-b border-gray-200 dark:border-slate-800 text-[11px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
            <tr>
              <th class="py-2.5 px-3 w-8 text-center">
                <input
                  type="checkbox"
                  :checked="isAllSelected"
                  @change="toggleSelectAll"
                  class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
                />
              </th>
              <th class="py-2.5 px-2 cursor-pointer hover:text-gray-900 dark:hover:text-white" @click="setSort('name')">Name</th>
              <th class="py-2.5 px-2 w-28 text-right cursor-pointer hover:text-gray-900 dark:hover:text-white" @click="setSort('size')">Size</th>
              <th class="py-2.5 px-2 w-36 text-right cursor-pointer hover:text-gray-900 dark:hover:text-white" @click="setSort('modified')">Modified</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-100 dark:divide-slate-800/60 font-sans">
            <tr
              v-for="entry in displayedEntries"
              :key="entry.path"
              draggable="true"
              @dragstart="handleDragStart($event, entry)"
              @click="handleEntryClick($event, entry)"
              @dblclick="handleEntryDoubleClick(entry)"
              @contextmenu="openContextMenu($event, entry)"
              :class="[
                'cursor-pointer transition group',
                isItemHidden(entry) ? 'opacity-65 hover:opacity-100 italic' : '',
                panel.selectedEntries.includes(entry.path)
                  ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-900 dark:text-blue-100'
                  : 'hover:bg-gray-50/80 dark:hover:bg-slate-800/60 text-gray-800 dark:text-slate-200'
              ]"
            >
              <td class="py-2.5 px-3 text-center" @click.stop>
                <input
                  type="checkbox"
                  :checked="panel.selectedEntries.includes(entry.path)"
                  @change="toggleEntrySelect(entry.path, true)"
                  class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
                />
              </td>
              <td class="py-2.5 px-2 flex items-center space-x-3 truncate">
                <FbIcon
                  :name="entry.kind === 'directory' ? 'folder' : getCategoryIcon(entry)"
                  size="18px"
                  :class="isItemHidden(entry) ? 'text-gray-400 dark:text-slate-500' : (entry.kind === 'directory' ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400 dark:text-slate-500')"
                />
                <div class="truncate flex items-center space-x-1.5">
                  <span
                    class="truncate font-medium text-sm group-hover:text-blue-600 dark:group-hover:text-blue-400 transition"
                    :class="{ 'font-semibold': entry.kind === 'directory' }"
                  >
                    {{ entry.name }}
                  </span>
                  <span v-if="isItemHidden(entry)" class="text-[9px] px-1.5 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-400 dark:text-slate-500 font-mono not-italic">
                    dot
                  </span>
                </div>
              </td>
              <td class="py-2.5 px-2 text-right text-gray-500 dark:text-slate-400 font-mono text-xs">
                {{ entry.kind === 'directory' ? '—' : formatBytes(entry.size || 0) }}
              </td>
              <td class="py-2.5 px-2 text-right text-gray-400 dark:text-slate-500 text-xs truncate">
                {{ formatDate(entry.modified_at) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Filestash Floating Selection Action Bar -->
    <div
      v-if="panel.selectedEntries.length > 0"
      class="absolute bottom-6 left-1/2 transform -translate-x-1/2 z-40 bg-gray-900/95 dark:bg-slate-900/95 text-white backdrop-blur-md border border-gray-700/80 px-4 py-2 rounded-2xl shadow-2xl flex items-center space-x-3 text-xs animate-in slide-in-from-bottom-4 duration-150"
    >
      <span class="font-semibold text-blue-400 bg-blue-500/20 px-2.5 py-0.5 rounded-lg border border-blue-500/30">
        {{ panel.selectedEntries.length }} selected
      </span>

      <button
        @click="handleBatchCompress"
        class="hover:text-blue-300 flex items-center space-x-1 px-2 py-1 rounded-lg hover:bg-slate-800 transition cursor-pointer"
        title="Compress items"
      >
        <FbIcon name="archive" size="14px" />
        <span>Compress</span>
      </button>

      <button
        v-if="panel.selectedEntries.length === 1"
        @click="handleSingleRename"
        class="hover:text-blue-300 flex items-center space-x-1 px-2 py-1 rounded-lg hover:bg-slate-800 transition cursor-pointer"
        title="Rename item"
      >
        <FbIcon name="rename" size="14px" />
        <span>Rename</span>
      </button>

      <button
        @click="handleBatchDelete"
        class="text-red-400 hover:text-red-300 hover:bg-red-500/20 flex items-center space-x-1 px-2 py-1 rounded-lg transition cursor-pointer"
        title="Delete items"
      >
        <FbIcon name="delete" size="14px" />
        <span>Delete</span>
      </button>

      <button
        @click="panel.selectedEntries = []"
        class="text-gray-400 hover:text-white px-1.5 py-0.5 rounded text-sm ml-1 cursor-pointer"
        title="Clear Selection"
      >
        ✕
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import type { IconName } from '../../utils/icons';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useFileStore } from '../../stores/fileStore';
import { useTransferStore } from '../../stores/transferStore';
import { useUiStore } from '../../stores/uiStore';
import { apiClient } from '../../api/client';
import { getDownloadUrl } from '../../api/files';
import type { FileEntry } from '../../types/vfs';

const props = defineProps<{
  panelId: 'left' | 'right';
}>();

const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();
const fileStore = useFileStore();
const transferStore = useTransferStore();
const uiStore = useUiStore();

const panel = computed(() => workspaceStore.getPanel(props.panelId));
const isActive = computed(() => workspaceStore.activePanelId === props.panelId);
const isDragOver = ref(false);

const displayedEntries = computed(() => panel.value.entries);

const displayedFolders = computed(() => {
  return panel.value.entries.filter((e) => e.kind === 'directory');
});

const displayedFiles = computed(() => {
  return panel.value.entries.filter((e) => e.kind !== 'directory');
});

const isAllSelected = computed(() => {
  return (
    displayedEntries.value.length > 0 &&
    panel.value.selectedEntries.length === displayedEntries.value.length
  );
});

onMounted(async () => {
  await workspaceStore.fetchPanelEntries(props.panelId);
});

function isItemHidden(entry: FileEntry): boolean {
  return entry.is_hidden || entry.name.startsWith('.');
}

function handleConnectionChange(newConnId: string) {
  workspaceStore.switchPanelConnection(props.panelId, newConnId, '/');
}

function isImage(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'ico'].includes(ext);
}

function isVideo(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['mp4', 'webm', 'mov', 'avi', 'mkv', 'flv'].includes(ext);
}

function isAudio(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['mp3', 'wav', 'flac', 'aac', 'm4a', 'opus', 'ogg'].includes(ext);
}

function isCode(entry: FileEntry): boolean {
  return isTextOrCode(entry);
}

function isTextOrCode(entry: FileEntry): boolean {
  if (entry.name.startsWith('.')) return true; // All dotfiles are editable config/code/text!
  const ext = getFileExt(entry);
  const textExts = [
    'txt', 'md', 'log', 'env', 'json', 'yaml', 'yml', 'toml', 'xml', 'csv', 'tsv',
    'rs', 'ts', 'js', 'jsx', 'tsx', 'vue', 'html', 'css', 'scss', 'sass', 'less',
    'py', 'sh', 'bash', 'zsh', 'fish', 'c', 'cpp', 'h', 'hpp', 'go', 'java', 'kt',
    'php', 'rb', 'pl', 'lua', 'sql', 'conf', 'cfg', 'ini', 'properties', 'dockerfile',
    'lock', 'mod', 'sum', 'gradle', 'service', 'gitignore', 'gitattributes', 'npmrc',
    'bashrc', 'profile', 'zshrc', 'vimrc', 'eslintrc', 'prettierrc'
  ];
  return textExts.includes(ext) || ext === '';
}

function getFileExt(entry: FileEntry): string {
  return entry.name.split('.').pop()?.toLowerCase() || '';
}

function getCategoryIcon(entry: FileEntry): IconName {
  const ext = getFileExt(entry);
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar'].includes(ext)) return 'archive';
  if (['png', 'jpg', 'jpeg', 'gif', 'webp'].includes(ext)) return 'image';
  if (['mp4', 'webm', 'mov', 'mkv'].includes(ext)) return 'video';
  if (['mp3', 'wav', 'ogg', 'flac'].includes(ext)) return 'audio';
  if (['pdf'].includes(ext)) return 'pdf';
  if (isTextOrCode(entry)) return 'code';
  return 'file';
}

function toggleSelectAll() {
  if (isAllSelected.value) {
    panel.value.selectedEntries = [];
  } else {
    panel.value.selectedEntries = displayedEntries.value.map((e) => e.path);
  }
}

function setSort(field: string) {
  if (panel.value.sortField === field) {
    panel.value.sortOrder = panel.value.sortOrder === 'asc' ? 'desc' : 'asc';
  } else {
    panel.value.sortField = field;
    panel.value.sortOrder = 'asc';
  }
  workspaceStore.fetchPanelEntries(props.panelId);
}

function toggleEntrySelect(path: string, multi: boolean = false) {
  if (!multi) {
    panel.value.selectedEntries = panel.value.selectedEntries.includes(path) ? [] : [path];
  } else {
    if (panel.value.selectedEntries.includes(path)) {
      panel.value.selectedEntries = panel.value.selectedEntries.filter((p) => p !== path);
    } else {
      panel.value.selectedEntries.push(path);
    }
  }
}

function handleEntryClick(e: MouseEvent, entry: FileEntry) {
  toggleEntrySelect(entry.path, e.ctrlKey || e.metaKey);
}

async function handleEntryDoubleClick(entry: FileEntry) {
  if (entry.kind === 'directory') {
    workspaceStore.navigatePanel(props.panelId, entry.path);
    return;
  }

  const ext = getFileExt(entry);
  const isMedia = ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico', 'avif', 'mp4', 'webm', 'mov', 'mkv', 'avi', 'mp3', 'wav', 'flac', 'aac', 'm4a', 'opus', 'ogg'].includes(ext);
  const url = getDownloadUrl(panel.value.connectionId, entry.path);

  if (isMedia) {
    uiStore.openMediaViewer(entry.name, url, entry, displayedFiles.value, panel.value.connectionId);
  } else {
    // Open in Code Editor for dotfiles, text files, and configs
    try {
      fileStore.currentConnectionId = panel.value.connectionId;
      const resp = await apiClient.get(`/connections/${panel.value.connectionId}/files/content`, {
        params: { path: entry.path },
        responseType: 'text',
      });
      uiStore.openEditor(entry, resp.data, resp.headers['etag'] || '');
    } catch {
      window.open(url, '_blank');
    }
  }
}

function openContextMenu(e: MouseEvent, entry: FileEntry) {
  fileStore.currentConnectionId = panel.value.connectionId;
  fileStore.currentPath = panel.value.path;
  uiStore.openContextMenu(e, entry);
}

function handleBatchCompress() {
  fileStore.currentConnectionId = panel.value.connectionId;
  fileStore.currentPath = panel.value.path;
}

function handleSingleRename() {
  const selectedPath = panel.value.selectedEntries[0];
  const selectedEntry = panel.value.entries.find((e) => e.path === selectedPath);
  if (selectedEntry) {
    fileStore.currentConnectionId = panel.value.connectionId;
    fileStore.currentPath = panel.value.path;
    uiStore.openRename(selectedEntry);
  }
}

function handleBatchDelete() {
  fileStore.currentConnectionId = panel.value.connectionId;
  fileStore.currentPath = panel.value.path;
  uiStore.openDelete(panel.value.selectedEntries);
}

function handleDragStart(e: DragEvent, entry: FileEntry) {
  const selected = panel.value.selectedEntries.includes(entry.path)
    ? panel.value.selectedEntries
    : [entry.path];

  const payload = {
    sourcePanelId: props.panelId,
    sourceConnectionId: panel.value.connectionId,
    paths: selected,
  };

  e.dataTransfer?.setData('application/json', JSON.stringify(payload));
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'copyMove';
  }
}

async function handleDrop(e: DragEvent) {
  isDragOver.value = false;
  const rawData = e.dataTransfer?.getData('application/json');
  if (!rawData) return;

  try {
    const data = JSON.parse(rawData);
    if (data.sourcePanelId !== props.panelId) {
      for (const filePath of data.paths) {
        const fileName = filePath.split('/').pop() || 'file';
        const targetPath = panel.value.path === '/' ? `/${fileName}` : `${panel.value.path}/${fileName}`;

        await transferStore.submitTransfer(
          `Copy ${fileName} to ${panel.value.path}`,
          'copy',
          data.sourceConnectionId,
          filePath,
          panel.value.connectionId,
          targetPath
        );
      }
      uiStore.showToast(`Queued ${data.paths.length} transfer(s)`, 'info');
      setTimeout(() => {
        workspaceStore.fetchPanelEntries(props.panelId);
      }, 1000);
    }
  } catch (err: any) {
    uiStore.showToast('Transfer queue failed', 'error');
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

function formatRelativeTime(dateStr?: string): string {
  if (!dateStr) return 'recently';
  const d = new Date(dateStr);
  const now = new Date();
  const diffSec = Math.floor((now.getTime() - d.getTime()) / 1000);

  if (diffSec < 60) return 'just now';
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)} mins ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)} hours ago`;
  if (diffSec < 2592000) return `${Math.floor(diffSec / 86400)} days ago`;
  const months = Math.floor(diffSec / 2592000);
  return `${months} ${months === 1 ? 'month' : 'months'} ago`;
}

function formatDate(dateStr?: string): string {
  if (!dateStr) return '—';
  const d = new Date(dateStr);
  return d.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}
</script>
