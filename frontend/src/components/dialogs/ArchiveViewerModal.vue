<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 bg-black/75 backdrop-blur-xs flex items-center justify-center p-3 sm:p-6 select-none font-sans text-xs animate-in fade-in duration-150"
    @click="close"
  >
    <div
      class="bg-white dark:bg-[#0c101c] border border-gray-200 dark:border-slate-800 rounded-2xl max-w-3xl w-full flex flex-col shadow-2xl overflow-hidden h-[580px] max-h-[88vh] animate-in zoom-in-95 duration-150"
      @click.stop
    >
      <!-- Header -->
      <div class="h-13 bg-gray-50/70 dark:bg-[#090d18] border-b border-gray-200 dark:border-slate-800/80 px-4 flex items-center justify-between text-xs shrink-0">
        <div class="flex items-center space-x-2.5 truncate">
          <div class="w-7 h-7 rounded-lg bg-amber-500/10 text-amber-500 dark:text-amber-400 flex items-center justify-center shrink-0">
            <FbIcon name="archive" size="15px" />
          </div>
          <div class="truncate">
            <div class="flex items-center space-x-2 truncate">
              <h3 class="text-xs sm:text-[13px] font-semibold text-gray-900 dark:text-slate-100 truncate">{{ archiveName }}</h3>
              <span class="px-1.5 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-600 dark:text-slate-400 font-mono text-[9px] uppercase font-semibold">
                {{ archiveExt }}
              </span>
            </div>
            <p class="text-[10px] text-gray-400 dark:text-slate-500 font-mono truncate">
              {{ connectionId }}:{{ archivePath }}
            </p>
          </div>
        </div>

        <div class="flex items-center space-x-1">
          <button
            @click="fetchEntries"
            :disabled="loading"
            class="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
            title="Refresh Archive"
          >
            <FbIcon name="refresh" size="14px" :class="{ 'animate-spin': loading }" />
          </button>

          <button
            @click="close"
            class="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
            title="Close"
          >
            <FbIcon name="x" size="15px" />
          </button>
        </div>
      </div>

      <!-- Navigation & Breadcrumbs Subbar -->
      <div class="h-10 bg-white dark:bg-[#0c101c] border-b border-gray-200 dark:border-slate-800/80 px-4 flex items-center justify-between gap-3 text-xs shrink-0">
        <!-- Breadcrumb Navigation -->
        <div class="flex items-center space-x-1 overflow-x-auto truncate flex-1 py-1 scrollbar-none text-[11px]">
          <button
            @click="navigateTo('')"
            :class="[
              'px-2 py-0.5 rounded-md transition font-medium flex items-center space-x-1 cursor-pointer shrink-0',
              currentSubpath === ''
                ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 font-semibold'
                : 'text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
            ]"
          >
            <FbIcon name="home" size="12px" />
            <span>Root</span>
          </button>

          <template v-for="(seg, idx) in breadcrumbSegments" :key="idx">
            <span class="text-gray-300 dark:text-slate-700 select-none">
              <FbIcon name="chevron-right" size="11px" />
            </span>
            <button
              @click="navigateTo(seg.subpath)"
              :class="[
                'px-2 py-0.5 rounded-md transition font-medium cursor-pointer shrink-0 truncate max-w-[120px]',
                idx === breadcrumbSegments.length - 1
                  ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 font-semibold'
                  : 'text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
              ]"
            >
              {{ seg.name }}
            </button>
          </template>
        </div>

        <!-- Filter Input -->
        <div class="w-36 sm:w-48 shrink-0 relative flex items-center">
          <FbIcon name="search" size="12px" class="absolute left-2.5 text-gray-400 dark:text-slate-500 pointer-events-none" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Search in archive..."
            class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-lg pl-7 pr-6 py-1 text-[11px] text-gray-900 dark:text-white outline-none focus:border-amber-500 transition font-medium placeholder-gray-400 dark:placeholder-slate-500"
          />
          <button
            v-if="searchQuery"
            @click="searchQuery = ''"
            class="absolute right-2 text-gray-400 hover:text-gray-600 dark:hover:text-slate-300 cursor-pointer text-[10px]"
          >
            ✕
          </button>
        </div>
      </div>

      <!-- Main Content Table Area -->
      <div class="flex-1 overflow-y-auto relative">
        <!-- Loading State -->
        <div v-if="loading" class="h-full flex items-center justify-center text-gray-400 space-x-2">
          <div class="animate-spin rounded-full h-4 w-4 border-2 border-amber-500 border-t-transparent"></div>
          <span class="text-xs">Reading archive contents...</span>
        </div>

        <!-- Error State -->
        <div v-else-if="error" class="h-full flex flex-col items-center justify-center p-6 text-center space-y-2.5">
          <div class="w-10 h-10 rounded-xl bg-red-500/10 text-red-500 flex items-center justify-center">
            <FbIcon name="info" size="20px" />
          </div>
          <div>
            <p class="font-semibold text-xs text-gray-900 dark:text-white">Failed to read archive</p>
            <p class="text-[11px] text-gray-500 dark:text-slate-400 max-w-sm mt-0.5">{{ error }}</p>
          </div>
          <button
            @click="fetchEntries"
            class="px-3 py-1 rounded-lg bg-amber-500 hover:bg-amber-600 text-white font-medium cursor-pointer transition text-xs shadow-xs"
          >
            Retry
          </button>
        </div>

        <!-- Empty State -->
        <div v-else-if="filteredEntries.length === 0" class="h-full flex flex-col items-center justify-center p-6 text-gray-400 space-y-1.5">
          <FbIcon name="empty-folder" size="28px" class="opacity-50" />
          <span class="text-xs text-gray-400 dark:text-slate-500">Folder is empty</span>
        </div>

        <!-- Virtual Entries Table -->
        <table v-else class="w-full text-left border-collapse text-xs select-none">
          <thead class="sticky top-0 z-10 bg-gray-50/95 dark:bg-[#090d18]/95 backdrop-blur-xs border-b border-gray-200 dark:border-slate-800/80 text-[10px] font-semibold text-gray-400 dark:text-slate-500 uppercase tracking-wider font-mono">
            <tr>
              <th class="py-1.5 px-3 w-8 text-center">
                <input
                  type="checkbox"
                  :checked="isAllSelected"
                  @change="toggleSelectAll"
                  class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
                />
              </th>
              <th class="py-1.5 px-2">Name</th>
              <th class="py-1.5 px-3 w-24 text-right">Size</th>
              <th class="py-1.5 px-3 w-20 text-right">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-100 dark:divide-slate-800/40 font-sans">
            <!-- Parent navigation (..) if inside subpath -->
            <tr
              v-if="currentSubpath !== ''"
              @click="navigateUp"
              class="cursor-pointer transition hover:bg-gray-50 dark:hover:bg-slate-800/40 text-gray-700 dark:text-slate-300 group"
            >
              <td class="py-1.5 px-3 text-center"></td>
              <td class="py-1.5 px-2 flex items-center space-x-2 text-amber-600 dark:text-amber-400 font-medium">
                <FbIcon name="folder" size="15px" />
                <span>.. (Parent Directory)</span>
              </td>
              <td class="py-1.5 px-3 text-right text-gray-400 font-mono text-[11px]">—</td>
              <td class="py-1.5 px-3 text-right"></td>
            </tr>

            <tr
              v-for="entry in filteredEntries"
              :key="entry.path"
              @click="handleRowClick($event, entry)"
              @dblclick="handleRowDoubleClick(entry)"
              :class="[
                'cursor-pointer transition group',
                selectedPaths.includes(entry.path)
                  ? 'bg-blue-500/10 text-blue-900 dark:text-blue-200 border-l-2 border-l-blue-500'
                  : 'hover:bg-gray-50 dark:hover:bg-slate-800/40 text-gray-800 dark:text-slate-200 border-l-2 border-l-transparent'
              ]"
            >
              <td class="py-1.5 px-3 text-center" @click.stop>
                <input
                  type="checkbox"
                  :checked="selectedPaths.includes(entry.path)"
                  @change="toggleSelect(entry.path)"
                  class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
                />
              </td>

              <td class="py-1.5 px-2 flex items-center space-x-2 truncate">
                <FbIcon
                  :name="getEntryIcon(entry)"
                  size="15px"
                  :class="[
                    'shrink-0',
                    entry.kind === 'directory' ? 'text-amber-500 dark:text-amber-400' : 'text-gray-400 dark:text-slate-400'
                  ]"
                />
                <span :class="['truncate text-xs', entry.kind === 'directory' ? 'font-semibold text-gray-900 dark:text-white' : 'font-medium']">
                  {{ entry.name }}
                </span>
              </td>

              <td class="py-1.5 px-3 text-right font-mono text-[11px] text-gray-500 dark:text-slate-400 truncate">
                {{ entry.kind === 'directory' ? '—' : formatBytes(entry.size) }}
              </td>

              <td class="py-1.5 px-3 text-right space-x-1 shrink-0">
                <template v-if="entry.kind === 'file'">
                  <button
                    v-if="isTextFile(entry.name)"
                    @click.stop="previewFile(entry)"
                    class="p-1 rounded text-gray-400 hover:text-blue-500 hover:bg-gray-100 dark:hover:bg-slate-700 transition cursor-pointer inline-flex items-center justify-center"
                    title="Preview File"
                  >
                    <FbIcon name="open" size="13px" />
                  </button>
                  <a
                    :href="getDownloadUrl(entry)"
                    download
                    @click.stop
                    class="p-1 rounded text-gray-400 hover:text-blue-500 hover:bg-gray-100 dark:hover:bg-slate-700 transition inline-flex items-center justify-center cursor-pointer"
                    title="Download Entry"
                  >
                    <FbIcon name="download" size="13px" />
                  </a>
                </template>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Footer Bar with Summary & Action Buttons -->
      <div class="h-12 bg-gray-50/70 dark:bg-[#090d18] border-t border-gray-200 dark:border-slate-800/80 px-4 flex items-center justify-between text-xs shrink-0 select-none">
        <div class="flex items-center space-x-2 text-[11px] text-gray-500 dark:text-slate-400">
          <span>{{ entries.length }} item{{ entries.length === 1 ? '' : 's' }}</span>
          <span v-if="totalSize > 0" class="font-mono">({{ formatBytes(totalSize) }})</span>
          <span v-if="selectedPaths.length > 0" class="text-blue-600 dark:text-blue-400 font-semibold">
            • {{ selectedPaths.length }} selected
          </span>
        </div>

        <div class="flex items-center space-x-2">
          <button
            @click="close"
            class="px-3 py-1.5 rounded-lg text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition text-xs font-medium cursor-pointer"
          >
            Close
          </button>

          <button
            v-if="selectedPaths.length > 0"
            @click="extractSelected"
            :disabled="extracting"
            class="px-3 py-1.5 rounded-lg bg-amber-500 hover:bg-amber-600 text-white font-medium flex items-center space-x-1.5 shadow-xs transition cursor-pointer disabled:opacity-50 text-xs"
          >
            <span v-if="extracting" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <span>Extract Selected ({{ selectedPaths.length }})</span>
          </button>

          <button
            @click="extractAll"
            :disabled="extracting"
            class="px-3.5 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white font-medium flex items-center space-x-1.5 shadow-xs transition cursor-pointer disabled:opacity-50 text-xs"
          >
            <span v-if="extracting && selectedPaths.length === 0" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <span>Extract All</span>
          </button>
        </div>
      </div>
    </div>
  </div>
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
