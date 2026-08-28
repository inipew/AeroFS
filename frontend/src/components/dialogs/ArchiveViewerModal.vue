<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/80 backdrop-blur-xs flex items-center justify-center p-3 sm:p-6 select-none font-sans text-xs"
      @click="close"
    >
      <div
        class="modal-card bg-white dark:bg-[#0c101c] border border-gray-200 dark:border-slate-800/90 rounded-2xl sm:rounded-3xl max-w-4xl w-full flex flex-col shadow-2xl overflow-hidden min-h-[380px] max-h-[85vh]"
        @click.stop
      >
      <!-- Header with Gradient Accent -->
      <div class="bg-gray-50/90 dark:bg-[#090d18]/90 border-b border-gray-200 dark:border-slate-800/80 px-4 sm:px-5 py-3 flex items-center justify-between gap-3 text-xs shrink-0">
        <!-- Archive Identity -->
        <div class="flex items-center space-x-3 truncate">
          <div class="w-10 h-10 rounded-2xl bg-gradient-to-br from-amber-500/20 to-orange-500/10 border border-amber-500/20 text-amber-500 dark:text-amber-400 flex items-center justify-center shrink-0 shadow-inner">
            <FbIcon name="archive" size="20px" />
          </div>

          <div class="truncate">
            <div class="flex items-center space-x-2 truncate">
              <h3 class="text-xs sm:text-sm font-bold text-gray-900 dark:text-slate-100 truncate tracking-tight">
                {{ archiveName }}
              </h3>
              <span class="px-2 py-0.5 rounded-md bg-blue-500/10 dark:bg-blue-900/30 border border-blue-500/20 text-blue-600 dark:text-blue-400 font-mono text-[10px] uppercase font-bold tracking-wide">
                {{ archiveExt }}
              </span>
              <span v-if="totalSize > 0" class="hidden sm:inline-block px-2 py-0.5 rounded-md bg-gray-100 dark:bg-slate-800/80 text-gray-500 dark:text-slate-400 font-mono text-[10px]">
                {{ formatBytes(totalSize) }}
              </span>
            </div>

            <p class="text-[11px] text-gray-400 dark:text-slate-500 font-mono truncate mt-0.5">
              <span class="text-gray-500 dark:text-slate-400 font-semibold">{{ connectionId }}</span>:{{ archivePath }}
            </p>
          </div>
        </div>

        <!-- Header Actions -->
        <div class="flex items-center space-x-2 shrink-0">
          <button
            @click="extractAll"
            :disabled="extracting || entries.length === 0"
            class="px-3 sm:px-4 py-1.5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-semibold flex items-center space-x-1.5 shadow-sm transition cursor-pointer disabled:opacity-50 text-xs active:scale-95"
            title="Extract all files to the current directory"
          >
            <span v-if="extracting && selectedPaths.length === 0" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <FbIcon v-else name="download" size="13px" />
            <span class="hidden sm:inline">Extract All</span>
            <span class="sm:hidden">Extract</span>
          </button>

          <button
            @click="fetchEntries"
            :disabled="loading"
            class="p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
            title="Refresh Archive"
          >
            <FbIcon name="refresh" size="14px" :class="{ 'animate-spin': loading }" />
          </button>

          <button
            @click="close"
            class="p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
            title="Close"
          >
            <FbIcon name="x" size="15px" />
          </button>
        </div>
      </div>

      <!-- Navigation & Breadcrumbs Bar -->
      <div class="h-11 bg-white dark:bg-[#0c101c] border-b border-gray-200 dark:border-slate-800/80 px-4 sm:px-5 flex items-center justify-between gap-3 text-xs shrink-0">
        <!-- Breadcrumb Path Segment -->
        <div class="flex items-center space-x-1 overflow-x-auto truncate flex-1 py-1 scrollbar-none text-[11px]">
          <button
            @click="navigateTo('')"
            :class="[
              'px-2.5 py-1 rounded-lg transition font-medium flex items-center space-x-1.5 cursor-pointer shrink-0',
              currentSubpath === ''
                ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 font-bold'
                : 'text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800/80 hover:text-gray-900 dark:hover:text-slate-200'
            ]"
          >
            <FbIcon name="home" size="13px" />
            <span>Root</span>
          </button>

          <template v-for="(seg, idx) in breadcrumbSegments" :key="idx">
            <span class="text-gray-300 dark:text-slate-700 select-none">
              <FbIcon name="chevron-right" size="11px" />
            </span>
            <button
              @click="navigateTo(seg.subpath)"
              :class="[
                'px-2.5 py-1 rounded-lg transition font-medium cursor-pointer shrink-0 truncate max-w-[140px]',
                idx === breadcrumbSegments.length - 1
                  ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 font-bold'
                  : 'text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800/80 hover:text-gray-900 dark:hover:text-slate-200'
              ]"
            >
              {{ seg.name }}
            </button>
          </template>
        </div>

        <!-- Inline Quick Search -->
        <div class="w-40 sm:w-56 shrink-0 relative flex items-center">
          <FbIcon name="search" size="13px" class="absolute left-2.5 text-gray-400 dark:text-slate-500 pointer-events-none" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Search entries..."
            class="w-full bg-gray-50 dark:bg-slate-900/80 border border-gray-200 dark:border-slate-800 rounded-xl pl-7.5 pr-6 py-1.5 text-[11px] text-gray-900 dark:text-slate-200 outline-none focus:border-amber-500/80 focus:ring-1 focus:ring-amber-500/30 transition font-medium placeholder-gray-400 dark:placeholder-slate-500"
          />
          <button
            v-if="searchQuery"
            @click="searchQuery = ''"
            class="absolute right-2.5 text-gray-400 hover:text-gray-600 dark:hover:text-slate-300 cursor-pointer text-[10px]"
          >
            ✕
          </button>
        </div>
      </div>

      <!-- Main Content Area -->
      <div class="flex-1 overflow-y-auto relative p-2 sm:p-3">
        <!-- Loading State -->
        <div v-if="loading" class="py-20 flex flex-col items-center justify-center text-gray-400 dark:text-slate-500 space-y-2">
          <div class="animate-spin rounded-full h-6 w-6 border-2 border-amber-500 border-t-transparent"></div>
          <span class="text-xs font-medium">Scanning virtual archive headers...</span>
        </div>

        <!-- Error State -->
        <div v-else-if="error" class="py-16 flex flex-col items-center justify-center p-6 text-center space-y-3">
          <div class="w-11 h-11 rounded-2xl bg-red-500/10 text-red-500 flex items-center justify-center border border-red-500/20">
            <FbIcon name="info" size="22px" />
          </div>
          <div>
            <p class="font-bold text-sm text-gray-900 dark:text-white">Unable to read archive</p>
            <p class="text-[11px] text-gray-500 dark:text-slate-400 max-w-sm mt-1 font-mono">{{ error }}</p>
          </div>
          <button
            @click="fetchEntries"
            class="px-4 py-1.5 rounded-xl bg-amber-500 hover:bg-amber-600 text-white font-semibold cursor-pointer transition text-xs shadow-xs"
          >
            Retry
          </button>
        </div>

        <!-- Empty State -->
        <div v-else-if="filteredEntries.length === 0" class="py-16 flex flex-col items-center justify-center text-gray-400 dark:text-slate-500 space-y-2">
          <div class="w-12 h-12 rounded-2xl bg-gray-100 dark:bg-slate-900 flex items-center justify-center text-gray-400 dark:text-slate-600">
            <FbIcon name="empty-folder" size="24px" />
          </div>
          <span class="text-xs font-medium">No items found in this directory</span>
        </div>

        <!-- Virtual Entries List -->
        <div v-else class="space-y-1">
          <!-- Table Header -->
          <div class="px-3 py-2 flex items-center justify-between text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider font-mono border-b border-gray-100 dark:border-slate-800/60 select-none">
            <div class="flex items-center space-x-3 flex-1 truncate">
              <input
                type="checkbox"
                :checked="isAllSelected"
                @change="toggleSelectAll"
                class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer w-3.5 h-3.5"
              />
              <span>NAME</span>
            </div>
            <div class="flex items-center space-x-6 shrink-0 pr-2">
              <span class="w-20 text-right">SIZE</span>
              <span class="w-24 text-right">ACTIONS</span>
            </div>
          </div>

          <!-- Parent navigation (..) if inside subpath -->
          <div
            v-if="currentSubpath !== ''"
            @click="navigateUp"
            class="p-2.5 rounded-xl hover:bg-gray-50 dark:hover:bg-slate-800/50 cursor-pointer flex items-center justify-between transition group text-xs border border-transparent hover:border-gray-200 dark:hover:border-slate-700/60"
          >
            <div class="flex items-center space-x-3 truncate">
              <div class="w-3.5"></div>
              <div class="w-7 h-7 rounded-lg bg-amber-500/10 text-amber-500 flex items-center justify-center shrink-0">
                <FbIcon name="folder" size="14px" />
              </div>
              <span class="font-bold text-amber-600 dark:text-amber-400">.. (Parent Directory)</span>
            </div>
            <span class="text-gray-400 dark:text-slate-600 font-mono text-[10px]">—</span>
          </div>

          <!-- Entry Row Cards -->
          <div
            v-for="entry in filteredEntries"
            :key="entry.path"
            @click="handleRowClick($event, entry)"
            @dblclick="handleRowDoubleClick(entry)"
            :class="[
              'p-2 sm:p-2.5 rounded-xl cursor-pointer flex items-center justify-between transition group text-xs border select-none',
              selectedPaths.includes(entry.path)
                ? 'bg-blue-50/80 dark:bg-blue-950/40 border-blue-500/50 shadow-xs'
                : 'bg-transparent hover:bg-gray-50 dark:hover:bg-slate-900/60 border-transparent hover:border-gray-200 dark:hover:border-slate-800/80'
            ]"
          >
            <!-- Left: Checkbox + Icon Badge + Name + Format -->
            <div class="flex items-center space-x-3 truncate flex-1 min-w-0 pr-2">
              <div @click.stop class="shrink-0 flex items-center">
                <input
                  type="checkbox"
                  :checked="selectedPaths.includes(entry.path)"
                  @change="toggleSelect(entry.path)"
                  class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer w-3.5 h-3.5"
                />
              </div>

              <div
                :class="[
                  'w-7 h-7 rounded-lg flex items-center justify-center shrink-0 transition shadow-inner',
                  getBadgeStyle(entry)
                ]"
              >
                <FbIcon :name="getEntryIcon(entry)" size="14px" />
              </div>

              <div class="truncate flex items-center space-x-2">
                <span :class="['truncate text-xs', entry.kind === 'directory' ? 'font-bold text-gray-900 dark:text-white' : 'font-medium text-gray-800 dark:text-slate-200']">
                  {{ entry.name }}
                </span>

                <span v-if="entry.kind === 'file'" class="hidden md:inline-block text-[9px] px-1.5 py-0.2 rounded bg-gray-100 dark:bg-slate-800/80 text-gray-400 dark:text-slate-500 font-mono uppercase">
                  {{ getFileExt(entry.name) }}
                </span>
              </div>
            </div>

            <!-- Right: Size + Action Buttons -->
            <div class="flex items-center space-x-6 shrink-0">
              <!-- Size -->
              <span class="w-20 text-right font-mono text-[11px] text-gray-500 dark:text-slate-400">
                {{ entry.kind === 'directory' ? '—' : formatBytes(entry.size) }}
              </span>

              <!-- Action Icons -->
              <div class="w-24 flex items-center justify-end space-x-1 shrink-0">
                <template v-if="entry.kind === 'file'">
                  <!-- Text / Code Preview Button -->
                  <button
                    v-if="isTextFile(entry.name)"
                    @click.stop="previewFile(entry)"
                    class="p-1.5 rounded-lg text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-950/50 transition cursor-pointer inline-flex items-center justify-center"
                    title="Preview File in Code Editor"
                  >
                    <FbIcon name="open" size="13px" />
                  </button>

                  <!-- Direct Download Button -->
                  <a
                    :href="getDownloadUrl(entry)"
                    download
                    @click.stop
                    class="p-1.5 rounded-lg text-gray-400 hover:text-emerald-600 dark:hover:text-emerald-400 hover:bg-emerald-50 dark:hover:bg-emerald-950/50 transition inline-flex items-center justify-center cursor-pointer"
                    title="Direct Download"
                  >
                    <FbIcon name="download" size="13px" />
                  </a>
                </template>

                <!-- Single Item Extract Button -->
                <button
                  @click.stop="extractSingle(entry)"
                  class="p-1.5 rounded-lg text-gray-400 hover:text-amber-600 dark:hover:text-amber-400 hover:bg-amber-50 dark:hover:bg-amber-950/50 transition inline-flex items-center justify-center cursor-pointer"
                  title="Extract this item"
                >
                  <FbIcon name="archive" size="13px" />
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer Bar with Metrics & Action Buttons -->
      <div class="bg-gray-50/90 dark:bg-[#090d18]/90 border-t border-gray-200 dark:border-slate-800/80 px-4 sm:px-5 py-2.5 flex items-center justify-between text-xs shrink-0 select-none">
        <!-- Status summary -->
        <div class="flex items-center space-x-2 text-[11px] text-gray-500 dark:text-slate-400">
          <span class="font-medium text-gray-700 dark:text-slate-300">{{ entries.length }} item{{ entries.length === 1 ? '' : 's' }}</span>
          <span v-if="totalSize > 0" class="font-mono">({{ formatBytes(totalSize) }})</span>
          <span v-if="selectedPaths.length > 0" class="text-blue-600 dark:text-blue-400 font-bold bg-blue-50 dark:bg-blue-950/50 px-2 py-0.5 rounded-md border border-blue-500/20">
            {{ selectedPaths.length }} selected
          </span>
        </div>

        <!-- Action Buttons -->
        <div class="flex items-center space-x-2">
          <button
            v-if="selectedPaths.length > 0"
            @click="extractSelected"
            :disabled="extracting"
            class="px-3.5 py-1.5 rounded-xl bg-amber-500 hover:bg-amber-600 text-white font-semibold flex items-center space-x-1.5 shadow-sm transition cursor-pointer disabled:opacity-50 text-xs active:scale-95"
          >
            <span v-if="extracting" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <FbIcon v-else name="archive" size="13px" />
            <span>Extract Selected ({{ selectedPaths.length }})</span>
          </button>

          <button
            @click="close"
            class="px-4 py-1.5 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-200/60 dark:hover:bg-slate-800 transition text-xs font-medium cursor-pointer"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import type { IconName } from '../../utils/icons';
import {
  listArchiveEntriesApi,
  getArchiveEntryReadUrl,
  readArchiveEntryTextApi,
  extractSelectedArchiveApi,
  extractArchiveApi,
  type VirtualArchiveEntry,
} from '../../api/archive';
import { useUiStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';

const props = defineProps<{
  modelValue: boolean;
  connectionId: string;
  archivePath: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void;
}>();

const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();

const isOpen = ref(props.modelValue);
const loading = ref(false);
const extracting = ref(false);
const error = ref<string | null>(null);

const currentSubpath = ref('');
const searchQuery = ref('');
const entries = ref<VirtualArchiveEntry[]>([]);
const selectedPaths = ref<string[]>([]);

const archiveName = computed(() => {
  return props.archivePath.split('/').pop() || 'archive';
});

const archiveExt = computed(() => {
  const name = archiveName.value.toLowerCase();
  if (name.endsWith('.tar.gz')) return 'tar.gz';
  if (name.endsWith('.tgz')) return 'tgz';
  if (name.endsWith('.tar.bz2')) return 'tar.bz2';
  if (name.endsWith('.tar.xz')) return 'tar.xz';
  return name.split('.').pop() || 'zip';
});

const totalSize = computed(() => {
  return entries.value.reduce((acc, e) => acc + (e.size || 0), 0);
});

const breadcrumbSegments = computed(() => {
  if (!currentSubpath.value) return [];
  const parts = currentSubpath.value.split('/').filter(Boolean);
  let accumulated = '';
  return parts.map((part) => {
    accumulated = accumulated ? `${accumulated}/${part}` : part;
    return {
      name: part,
      subpath: accumulated,
    };
  });
});

const filteredEntries = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return entries.value;
  return entries.value.filter((e) => e.name.toLowerCase().includes(q));
});

const isAllSelected = computed(() => {
  return (
    filteredEntries.value.length > 0 &&
    filteredEntries.value.every((e) => selectedPaths.value.includes(e.path))
  );
});

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      currentSubpath.value = '';
      searchQuery.value = '';
      selectedPaths.value = [];
      fetchEntries();
    }
  }
);

async function fetchEntries() {
  if (!props.archivePath) return;
  loading.value = true;
  error.value = null;
  selectedPaths.value = [];
  try {
    entries.value = await listArchiveEntriesApi(
      props.connectionId,
      props.archivePath,
      currentSubpath.value
    );
  } catch (err: any) {
    error.value = err.response?.data?.error?.message || err.message || 'Failed to read archive';
  } finally {
    loading.value = false;
  }
}

function navigateTo(subpath: string) {
  currentSubpath.value = subpath;
  fetchEntries();
}

function navigateUp() {
  if (!currentSubpath.value) return;
  const parts = currentSubpath.value.split('/').filter(Boolean);
  parts.pop();
  currentSubpath.value = parts.join('/');
  fetchEntries();
}

function handleRowClick(_e: MouseEvent, entry: VirtualArchiveEntry) {
  if (selectedPaths.value.includes(entry.path)) {
    selectedPaths.value = selectedPaths.value.filter((p) => p !== entry.path);
  } else {
    selectedPaths.value = [entry.path];
  }
}

function handleRowDoubleClick(entry: VirtualArchiveEntry) {
  if (entry.kind === 'directory') {
    navigateTo(entry.path);
  } else if (isTextFile(entry.name)) {
    previewFile(entry);
  } else {
    const url = getDownloadUrl(entry);
    window.open(url, '_blank');
  }
}

function toggleSelect(path: string) {
  if (selectedPaths.value.includes(path)) {
    selectedPaths.value = selectedPaths.value.filter((p) => p !== path);
  } else {
    selectedPaths.value.push(path);
  }
}

function toggleSelectAll() {
  if (isAllSelected.value) {
    selectedPaths.value = [];
  } else {
    selectedPaths.value = filteredEntries.value.map((e) => e.path);
  }
}

function getDownloadUrl(entry: VirtualArchiveEntry): string {
  return getArchiveEntryReadUrl(props.connectionId, props.archivePath, entry.path);
}

function getFileExt(name: string): string {
  return name.split('.').pop() || 'file';
}

function isTextFile(name: string): boolean {
  if (name.startsWith('.')) return true;
  const ext = name.split('.').pop()?.toLowerCase() || '';
  const textExts = [
    'txt', 'md', 'log', 'env', 'json', 'yaml', 'yml', 'toml', 'xml', 'csv', 'tsv',
    'rs', 'ts', 'js', 'jsx', 'tsx', 'vue', 'html', 'css', 'scss', 'sass', 'less',
    'py', 'sh', 'bash', 'zsh', 'fish', 'c', 'cpp', 'h', 'hpp', 'go', 'java', 'kt',
    'php', 'rb', 'pl', 'lua', 'sql', 'conf', 'cfg', 'ini', 'properties', 'dockerfile',
    'lock', 'mod', 'sum', 'gradle', 'service', 'gitignore', 'gitattributes'
  ];
  return textExts.includes(ext);
}

function getEntryIcon(entry: VirtualArchiveEntry): IconName {
  if (entry.kind === 'directory') return 'folder';
  const ext = entry.name.split('.').pop()?.toLowerCase() || '';
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'image';
  if (['mp4', 'webm', 'mov', 'mkv'].includes(ext)) return 'video';
  if (['mp3', 'wav', 'ogg', 'flac'].includes(ext)) return 'audio';
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar'].includes(ext)) return 'archive';
  if (['pdf'].includes(ext)) return 'pdf';
  if (isTextFile(entry.name)) return 'code';
  return 'file';
}

function getBadgeStyle(entry: VirtualArchiveEntry): string {
  if (entry.kind === 'directory') {
    return 'bg-amber-500/10 text-amber-500 dark:text-amber-400 border border-amber-500/20';
  }
  const ext = entry.name.split('.').pop()?.toLowerCase() || '';
  if (isTextFile(entry.name)) {
    return 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20';
  }
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) {
    return 'bg-purple-500/10 text-purple-600 dark:text-purple-400 border border-purple-500/20';
  }
  if (['mp4', 'webm', 'mov', 'mkv'].includes(ext)) {
    return 'bg-rose-500/10 text-rose-600 dark:text-rose-400 border border-rose-500/20';
  }
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar'].includes(ext)) {
    return 'bg-orange-500/10 text-orange-600 dark:text-orange-400 border border-orange-500/20';
  }
  return 'bg-cyan-500/10 text-cyan-600 dark:text-cyan-400 border border-cyan-500/20';
}

async function previewFile(entry: VirtualArchiveEntry) {
  try {
    const text = await readArchiveEntryTextApi(props.connectionId, props.archivePath, entry.path);
    // Open in Code Editor modal for previewing!
    uiStore.openEditor({
      name: entry.name,
      path: `${props.archivePath}/${entry.path}`,
      kind: 'file',
      size: entry.size,
    } as any, text);
  } catch (err: any) {
    uiStore.showToast('Unable to preview file as text', 'info');
  }
}

async function extractSingle(entry: VirtualArchiveEntry) {
  extracting.value = true;
  try {
    const targetDir = props.archivePath.substring(0, props.archivePath.lastIndexOf('/')) || '/';
    await extractSelectedArchiveApi(
      props.connectionId,
      props.archivePath,
      targetDir,
      [entry.path]
    );
    uiStore.showToast(`Extracted ${entry.name}`, 'success');
    await workspaceStore.fetchPanelEntries(workspaceStore.activePanelId);
    close();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Extraction failed', 'error');
  } finally {
    extracting.value = false;
  }
}

async function extractSelected() {
  if (selectedPaths.value.length === 0) return;
  extracting.value = true;
  try {
    const targetDir = props.archivePath.substring(0, props.archivePath.lastIndexOf('/')) || '/';
    await extractSelectedArchiveApi(
      props.connectionId,
      props.archivePath,
      targetDir,
      selectedPaths.value
    );
    uiStore.showToast(`Extracted ${selectedPaths.value.length} item(s)`, 'success');
    await workspaceStore.fetchPanelEntries(workspaceStore.activePanelId);
    close();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Extraction failed', 'error');
  } finally {
    extracting.value = false;
  }
}

async function extractAll() {
  extracting.value = true;
  try {
    const targetDir = props.archivePath.substring(0, props.archivePath.lastIndexOf('/')) || '/';
    await extractArchiveApi(props.connectionId, props.archivePath, targetDir);
    uiStore.showToast('Archive extracted successfully', 'success');
    await workspaceStore.fetchPanelEntries(workspaceStore.activePanelId);
    close();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Extraction failed', 'error');
  } finally {
    extracting.value = false;
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KiB', 'MiB', 'GiB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function close() {
  isOpen.value = false;
  emit('update:modelValue', false);
}
</script>
