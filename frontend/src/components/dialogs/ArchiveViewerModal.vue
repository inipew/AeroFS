<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/80 backdrop-blur-xs flex items-center justify-center p-2 sm:p-4 md:p-6 select-none font-sans text-xs"
      @click="close"
    >
      <div
        class="modal-card bg-white dark:bg-[#111317] border border-gray-200/90 dark:border-white/[0.08] rounded-2xl sm:rounded-3xl w-[96vw] max-w-7xl h-[92vh] sm:h-[88vh] flex flex-col shadow-2xl overflow-hidden relative"
        @click.stop
      >
        <!-- Top Toolbar Header -->
        <header class="h-13 sm:h-14 bg-gray-50/90 dark:bg-[#0c0e12]/90 border-b border-gray-200/80 dark:border-white/[0.08] px-3.5 sm:px-5 flex items-center justify-between gap-3 text-xs shrink-0 backdrop-blur-md">
          <!-- Archive Title & Identifiers -->
          <div class="flex items-center space-x-3 truncate">
            <button
              v-if="currentSubpath !== ''"
              type="button"
              @click="navigateUp"
              class="p-1.5 rounded-xl border border-gray-200 dark:border-white/[0.08] hover:bg-gray-100 dark:hover:bg-white/[0.06] text-gray-600 dark:text-slate-300 transition cursor-pointer shrink-0"
              title="Go to parent directory"
            >
              <FbIcon name="arrow-up" size="14px" />
            </button>

            <div class="w-9 h-9 sm:w-10 sm:h-10 rounded-2xl bg-gradient-to-br from-amber-500/20 to-orange-500/10 border border-amber-500/25 text-amber-500 dark:text-amber-400 flex items-center justify-center shrink-0 shadow-xs">
              <FbIcon name="archive" size="19px" />
            </div>

            <div class="truncate">
              <div class="flex items-center space-x-2 truncate">
                <h3 class="text-xs sm:text-sm font-bold text-gray-900 dark:text-slate-100 truncate tracking-tight" :title="archiveName">
                  {{ archiveName }}
                </h3>
                <span class="px-2 py-0.5 rounded-md bg-amber-500/10 dark:bg-amber-900/30 border border-amber-500/20 text-amber-600 dark:text-amber-400 font-mono text-[10px] uppercase font-bold tracking-wide shrink-0">
                  {{ archiveExt }}
                </span>
                <span v-if="totalUnpackedSize > 0" class="hidden md:inline-block px-2 py-0.5 rounded-md bg-gray-200/60 dark:bg-white/[0.06] text-gray-500 dark:text-slate-400 font-mono text-[10px]">
                  {{ formatBytes(totalUnpackedSize) }}
                </span>
              </div>
              <p class="text-[11px] text-gray-400 dark:text-slate-500 font-mono truncate mt-0.5">
                <span class="text-gray-500 dark:text-slate-400 font-semibold">{{ connectionId }}</span>:{{ archivePath }}
              </p>
            </div>
          </div>

          <!-- Quick Search Filter in Header -->
          <div class="w-36 sm:w-56 md:w-72 shrink-0 relative flex items-center">
            <FbIcon name="search" size="13px" class="absolute left-3 text-gray-400 dark:text-slate-500 pointer-events-none" />
            <input
              v-model="searchQuery"
              type="text"
              placeholder="Search inside archive..."
              class="w-full bg-white dark:bg-[#161a22] border border-gray-200 dark:border-white/[0.08] rounded-xl pl-8.5 pr-7 py-1.5 text-[11px] text-gray-900 dark:text-slate-200 outline-none focus:border-blue-500 focus:ring-1 focus:ring-blue-500/30 transition placeholder-gray-400 dark:placeholder-slate-500 font-medium shadow-2xs"
            />
            <button
              v-if="searchQuery"
              type="button"
              @click="searchQuery = ''"
              class="absolute right-2.5 text-gray-400 hover:text-gray-600 dark:hover:text-slate-200 cursor-pointer text-[11px]"
            >
              ✕
            </button>
          </div>

          <!-- Header Action Buttons -->
          <div class="flex items-center space-x-1.5 sm:space-x-2 shrink-0">
            <!-- Extract Button -->
            <button
              type="button"
              @click="handleExtractClick"
              :disabled="loading || entries.length === 0"
              class="px-3.5 sm:px-4 py-1.5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-semibold flex items-center space-x-1.5 shadow-sm transition cursor-pointer disabled:opacity-50 text-xs active:scale-95 duration-150"
              :title="selectedPaths.length > 0 ? `Extract ${selectedPaths.length} selected items` : 'Extract all files to storage'"
            >
              <FbIcon name="download" size="13px" />
              <span>{{ selectedPaths.length > 0 ? `Extract (${selectedPaths.length})` : 'Extract All' }}</span>
            </button>

            <!-- Toggle Left Tree -->
            <button
              type="button"
              @click="showTreePane = !showTreePane"
              :class="[
                'p-2 rounded-xl border transition cursor-pointer hidden sm:flex items-center justify-center',
                showTreePane
                  ? 'border-blue-500/30 bg-blue-500/10 text-blue-600 dark:text-blue-400'
                  : 'border-gray-200 dark:border-white/[0.08] text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-white/[0.04]'
              ]"
              title="Toggle Folder Tree"
            >
              <FbIcon name="panel-left" size="14px" />
            </button>

            <!-- Toggle Right Inspector -->
            <button
              type="button"
              @click="showInspectorPane = !showInspectorPane"
              :class="[
                'p-2 rounded-xl border transition cursor-pointer hidden sm:flex items-center justify-center',
                showInspectorPane
                  ? 'border-blue-500/30 bg-blue-500/10 text-blue-600 dark:text-blue-400'
                  : 'border-gray-200 dark:border-white/[0.08] text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-white/[0.04]'
              ]"
              title="Toggle File Inspector"
            >
              <FbIcon name="panel-right" size="14px" />
            </button>

            <!-- Refresh Archive -->
            <button
              type="button"
              @click="refreshArchive"
              :disabled="loading"
              class="p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-white/[0.06] transition cursor-pointer disabled:opacity-50"
              title="Refresh archive contents"
            >
              <FbIcon name="refresh" size="14px" :class="{ 'animate-spin': loading }" />
            </button>

            <!-- Close Modal -->
            <button
              type="button"
              @click="close"
              class="p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-white/[0.06] transition cursor-pointer ml-1"
              title="Close (Esc)"
            >
              <FbIcon name="x" size="15px" />
            </button>
          </div>
        </header>

        <!-- Breadcrumb Navigation Bar -->
        <nav class="h-10 bg-white dark:bg-[#0e1117] border-b border-gray-200/80 dark:border-white/[0.08] px-4 sm:px-5 flex items-center justify-between text-xs shrink-0">
          <div class="flex items-center space-x-1 overflow-x-auto truncate flex-1 py-1 scrollbar-none text-[11px]">
            <!-- Root Button -->
            <button
              type="button"
              @click="navigateTo('')"
              :class="[
                'px-2.5 py-1 rounded-lg transition font-medium flex items-center space-x-1.5 cursor-pointer shrink-0',
                currentSubpath === ''
                  ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 font-bold'
                  : 'text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-white/[0.06] hover:text-gray-900 dark:hover:text-slate-200'
              ]"
            >
              <FbIcon name="home" size="13px" />
              <span>Root</span>
            </button>

            <!-- Breadcrumb Path Segments -->
            <template v-for="(seg, idx) in breadcrumbSegments" :key="idx">
              <span class="text-gray-300 dark:text-slate-700 select-none">
                <FbIcon name="chevron-right" size="11px" />
              </span>
              <button
                type="button"
                @click="navigateTo(seg.subpath)"
                :class="[
                  'px-2.5 py-1 rounded-lg transition font-medium cursor-pointer shrink-0 truncate max-w-[150px]',
                  idx === breadcrumbSegments.length - 1
                    ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 font-bold'
                    : 'text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-white/[0.06] hover:text-gray-900 dark:hover:text-slate-200'
                ]"
              >
                {{ seg.name }}
              </button>
            </template>
          </div>

          <!-- Total subitems indicator -->
          <div class="text-[11px] text-gray-400 dark:text-slate-500 font-mono hidden sm:block">
            {{ entries.length }} {{ entries.length === 1 ? 'item' : 'items' }} in this directory
          </div>
        </nav>

        <!-- 3-Pane Explorer Main Viewport -->
        <main class="flex-1 flex min-h-0 overflow-hidden relative">
          <!-- Left Pane: Interactive Folder Tree -->
          <ArchiveTreePane
            v-if="showTreePane"
            :connection-id="connectionId"
            :archive-path="archivePath"
            :current-subpath="currentSubpath"
            :root-folders="rootFolders"
            @navigate="navigateTo"
            @collapse="showTreePane = false"
            class="w-56 sm:w-64 shrink-0 hidden sm:flex"
          />

          <!-- Center Pane: Dense File Table -->
          <ArchiveFileTable
            :entries="entries"
            :selected-paths="selectedPaths"
            :active-entry="activeEntry"
            :search-query="searchQuery"
            :is-loading="loading"
            @select="handleRowSelect"
            @toggle-select="handleToggleSelect"
            @toggle-select-all="handleToggleSelectAll"
            @navigate="navigateTo"
            @open-preview="handleOpenPreview"
            class="flex-1 min-w-0"
          />

          <!-- Right Pane: Live Inspector & Preview -->
          <ArchiveInspectorPane
            v-if="showInspectorPane"
            :connection-id="connectionId"
            :archive-path="archivePath"
            :entry="activeEntry"
            @collapse="showInspectorPane = false"
            @extract="handleExtractSelected"
            class="w-72 sm:w-80 shrink-0 hidden md:flex"
          />

          <!-- Inline Extraction Confirmation Sheet -->
          <ArchiveExtractSheet
            :is-open="isExtractSheetOpen"
            :archive-name="archiveName"
            :default-destination="defaultDestination"
            :selected-paths="extractTargetPaths"
            :is-extracting="extracting"
            @close="isExtractSheetOpen = false"
            @extract="executeExtraction"
          />
        </main>

        <!-- Bottom Status Bar -->
        <footer class="h-8 bg-gray-50 dark:bg-[#0a0c10] border-t border-gray-200/80 dark:border-white/[0.08] px-4 sm:px-5 flex items-center justify-between text-[11px] text-gray-500 dark:text-slate-400 shrink-0 select-none font-mono">
          <!-- Total Archive Metrics -->
          <div class="flex items-center space-x-2 truncate">
            <span>{{ totalItemsCount }} {{ totalItemsCount === 1 ? 'item' : 'items' }}</span>
            <span class="text-gray-300 dark:text-slate-700">•</span>
            <span>{{ formatBytes(totalUnpackedSize) }} unpacked</span>
            <template v-if="totalCompressedSize > 0">
              <span class="text-gray-300 dark:text-slate-700">•</span>
              <span>{{ formatBytes(totalCompressedSize) }} compressed</span>
            </template>
            <template v-if="overallCompressionRatio > 0">
              <span class="text-gray-300 dark:text-slate-700">•</span>
              <span class="text-emerald-600 dark:text-emerald-400 font-bold">{{ overallCompressionRatio }}% saved</span>
            </template>
          </div>

          <!-- Selection & Connection Summary -->
          <div class="flex items-center space-x-2 shrink-0">
            <span v-if="selectedPaths.length > 0" class="text-blue-600 dark:text-blue-400 font-semibold">
              {{ selectedPaths.length }} selected
            </span>
            <span v-if="selectedPaths.length > 0" class="text-gray-300 dark:text-slate-700">•</span>
            <div class="flex items-center space-x-1 px-2 py-0.5 rounded-full bg-gray-200/60 dark:bg-white/[0.06] text-gray-600 dark:text-slate-300">
              <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
              <span class="text-[10px]">{{ connectionId }}</span>
            </div>
          </div>
        </footer>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import {
  listArchiveEntriesApi,
  extractSelectedArchiveApi,
  extractArchiveApi,
  type VirtualArchiveEntry,
  type ArchiveOverwriteMode,
} from '../../api/archive';
import { useUiStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useOverlayStore } from '../../overlays/overlayStore';
import { normalizeApiError } from '../../utils/errorNormalizer';

import ArchiveTreePane from './archive/ArchiveTreePane.vue';
import ArchiveFileTable from './archive/ArchiveFileTable.vue';
import ArchiveInspectorPane from './archive/ArchiveInspectorPane.vue';
import ArchiveExtractSheet from './archive/ArchiveExtractSheet.vue';

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
const overlayStore = useOverlayStore();

const isOpen = ref(props.modelValue);
const loading = ref(false);
const extracting = ref(false);
const error = ref<string | null>(null);

const currentSubpath = ref('');
const searchQuery = ref('');
const entries = ref<VirtualArchiveEntry[]>([]);
const rootFolders = ref<VirtualArchiveEntry[]>([]);
const selectedPaths = ref<string[]>([]);
const activeEntry = ref<VirtualArchiveEntry | null>(null);

// Responsive pane toggles (both open by default on desktop)
const showTreePane = ref(true);
const showInspectorPane = ref(true);

// Extraction sheet state
const isExtractSheetOpen = ref(false);
const extractTargetPaths = ref<string[]>([]);

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

const totalUnpackedSize = computed(() => {
  return entries.value.reduce((acc, e) => acc + (e.size || 0), 0);
});

const totalCompressedSize = computed(() => {
  return entries.value.reduce((acc, e) => acc + (e.compressed_size || e.size || 0), 0);
});

const overallCompressionRatio = computed(() => {
  if (totalUnpackedSize.value <= 0 || totalCompressedSize.value >= totalUnpackedSize.value) return 0;
  return Math.round(((totalUnpackedSize.value - totalCompressedSize.value) / totalUnpackedSize.value) * 100);
});

const totalItemsCount = computed(() => entries.value.length);

const defaultDestination = computed(() => {
  return props.archivePath.substring(0, props.archivePath.lastIndexOf('/')) || '/';
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

watch(
  () => [props.modelValue, props.archivePath, props.connectionId] as const,
  ([val, path]) => {
    isOpen.value = val;
    if (val && path) {
      currentSubpath.value = '';
      searchQuery.value = '';
      selectedPaths.value = [];
      activeEntry.value = null;
      void fetchEntries();
      void fetchRootFolders();
    }
  },
  { immediate: true }
);

async function fetchEntries() {
  if (!props.archivePath) return;
  loading.value = true;
  error.value = null;
  selectedPaths.value = [];
  try {
    const list = await listArchiveEntriesApi(
      props.connectionId,
      props.archivePath,
      currentSubpath.value
    );
    entries.value = list;

    // Auto-select first entry for instant inspector preview if available
    if (list.length > 0 && (!activeEntry.value || !list.some((e) => e.path === activeEntry.value?.path))) {
      activeEntry.value = list.find((e) => e.kind !== 'directory') || list[0];
    }
  } catch (err: any) {
    error.value = normalizeApiError(err).message;
    uiStore.showToast(error.value, 'error');
  } finally {
    loading.value = false;
  }
}

async function fetchRootFolders() {
  if (!props.archivePath) return;
  try {
    const list = await listArchiveEntriesApi(props.connectionId, props.archivePath, '');
    rootFolders.value = list.filter((i) => i.kind === 'directory');
  } catch {
    rootFolders.value = [];
  }
}

function refreshArchive() {
  void fetchEntries();
  void fetchRootFolders();
}

function navigateTo(subpath: string) {
  currentSubpath.value = subpath;
  void fetchEntries();
}

function navigateUp() {
  if (!currentSubpath.value) return;
  const parts = currentSubpath.value.split('/').filter(Boolean);
  parts.pop();
  currentSubpath.value = parts.join('/');
  void fetchEntries();
}

function handleRowSelect(entry: VirtualArchiveEntry, event: MouseEvent) {
  activeEntry.value = entry;

  if (event.ctrlKey || event.metaKey) {
    handleToggleSelect(entry.path);
  } else if (event.shiftKey && selectedPaths.value.length > 0) {
    const lastSelected = selectedPaths.value[selectedPaths.value.length - 1];
    const fromIdx = entries.value.findIndex((e) => e.path === lastSelected);
    const toIdx = entries.value.findIndex((e) => e.path === entry.path);
    if (fromIdx !== -1 && toIdx !== -1) {
      const min = Math.min(fromIdx, toIdx);
      const max = Math.max(fromIdx, toIdx);
      const range = entries.value.slice(min, max + 1).map((e) => e.path);
      selectedPaths.value = Array.from(new Set([...selectedPaths.value, ...range]));
    }
  } else {
    selectedPaths.value = [entry.path];
  }
}

function handleToggleSelect(path: string) {
  if (selectedPaths.value.includes(path)) {
    selectedPaths.value = selectedPaths.value.filter((p) => p !== path);
  } else {
    selectedPaths.value = [...selectedPaths.value, path];
  }
}

function handleToggleSelectAll() {
  if (selectedPaths.value.length === entries.value.length) {
    selectedPaths.value = [];
  } else {
    selectedPaths.value = entries.value.map((e) => e.path);
  }
}

function handleOpenPreview(entry: VirtualArchiveEntry) {
  activeEntry.value = entry;
  showInspectorPane.value = true;
}

function handleExtractClick() {
  if (selectedPaths.value.length > 0) {
    extractTargetPaths.value = [...selectedPaths.value];
  } else {
    extractTargetPaths.value = [];
  }
  isExtractSheetOpen.value = true;
}

function handleExtractSelected(paths: string[]) {
  extractTargetPaths.value = paths;
  isExtractSheetOpen.value = true;
}

async function executeExtraction(options: {
  destinationDir: string;
  createSubfolder: boolean;
  overwriteMode: ArchiveOverwriteMode;
}) {
  extracting.value = true;
  try {
    let target = options.destinationDir;
    if (options.createSubfolder) {
      const subName = archiveName.value.replace(/\.(zip|tar\.gz|tgz|tar\.bz2|tar\.xz|tar|7z|rar)$/i, '');
      target = target === '/' ? `/${subName}` : `${target}/${subName}`;
    }

    if (extractTargetPaths.value.length > 0) {
      await extractSelectedArchiveApi(
        props.connectionId,
        props.archivePath,
        target,
        extractTargetPaths.value,
        options.overwriteMode
      );
      uiStore.showToast(`Extracted ${extractTargetPaths.value.length} item(s) to ${target}`, 'success');
    } else {
      await extractArchiveApi(
        props.connectionId,
        props.archivePath,
        target,
        undefined,
        options.overwriteMode
      );
      uiStore.showToast(`Archive extracted successfully to ${target}`, 'success');
    }

    isExtractSheetOpen.value = false;
    await workspaceStore.refreshPanel(workspaceStore.activePanelId);
    close();
  } catch (err: any) {
    uiStore.showToast(normalizeApiError(err).message, 'error');
  } finally {
    extracting.value = false;
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function close() {
  if (isExtractSheetOpen.value) {
    isExtractSheetOpen.value = false;
    return;
  }
  isOpen.value = false;
  emit('update:modelValue', false);
  if (overlayStore.current?.type === 'archive-viewer') {
    overlayStore.close();
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && isOpen.value) {
    e.preventDefault();
    e.stopPropagation();
    close();
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
});
</script>
