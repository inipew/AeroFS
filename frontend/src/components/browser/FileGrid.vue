<template>
  <!--
    Scroll container for virtual grid.
    The virtualizer works row-by-row: each "row" contains `cols` file entries.
    This lets us virtualise a grid efficiently with a single vertical virtualizer.
  -->
  <div
    ref="scrollContainerRef"
    class="flex-1 overflow-auto bg-slate-950 p-4 select-none text-xs text-slate-300 relative"
  >
    <!-- Empty State -->
    <div v-if="filteredEntries.length === 0 && !fileStore.loading" class="py-16 text-center text-slate-600">
      <p class="text-base">Folder is empty</p>
      <p class="text-xs text-slate-700 mt-1">Upload or create a file to get started</p>
    </div>

    <!-- Loading State -->
    <div v-if="fileStore.loading" class="py-16 text-center text-slate-500">
      <div class="inline-block animate-spin rounded-full h-6 w-6 border-2 border-indigo-500 border-t-transparent mb-3"></div>
      <p>Loading files...</p>
    </div>

    <!--
      Virtual scroll container.
      Height = totalVirtualSize so the native scrollbar reflects real content height.
    -->
    <div
      v-if="!fileStore.loading && filteredEntries.length > 0"
      :style="{ height: `${totalVirtualSize}px`, position: 'relative' }"
    >
      <!-- Each virtual row holds `cols` cells -->
      <div
        v-for="vRow in virtualItems"
        :key="String(vRow.key)"
        :style="{
          position: 'absolute',
          top: 0,
          left: 0,
          width: '100%',
          height: `${vRow.size}px`,
          transform: `translateY(${vRow.start}px)`,
        }"
        class="grid gap-3"
        :class="gridClass"
      >
        <div
          v-for="colIdx in cols"
          :key="colIdx"
          v-show="rowEntries(vRow.index, colIdx - 1)"
        >
          <div
            v-if="rowEntries(vRow.index, colIdx - 1)"
            @click="handleClick($event, rowEntries(vRow.index, colIdx - 1)!)"
            @dblclick="handleDoubleClick(rowEntries(vRow.index, colIdx - 1)!)"
            @contextmenu="uiStore.openContextMenu($event, rowEntries(vRow.index, colIdx - 1)!)"
            :class="[
              'flex flex-col items-center p-3 rounded-lg border cursor-pointer transition text-center group',
              fileStore.selectedEntries.includes(rowEntries(vRow.index, colIdx - 1)!.path)
                ? 'bg-indigo-600/20 border-indigo-500/50 text-indigo-200'
                : 'bg-slate-900/40 border-slate-800/80 hover:bg-slate-900 hover:border-slate-700 text-slate-300'
            ]"
          >
            <!-- Icon -->
            <div class="text-3xl mb-2">
              <span v-if="rowEntries(vRow.index, colIdx - 1)!.kind === 'directory'">📁</span>
              <span v-else-if="rowEntries(vRow.index, colIdx - 1)!.kind === 'symlink'">🔗</span>
              <span v-else>📄</span>
            </div>

            <!-- Name -->
            <span
              class="w-full truncate font-medium text-xs group-hover:text-white"
              :title="rowEntries(vRow.index, colIdx - 1)!.name"
            >
              {{ rowEntries(vRow.index, colIdx - 1)!.name }}
            </span>

            <!-- Subtitle -->
            <span class="text-[10px] text-slate-500 mt-1">
              {{ rowEntries(vRow.index, colIdx - 1)!.kind === 'directory' ? 'Folder' : formatBytes(rowEntries(vRow.index, colIdx - 1)!.size || 0) }}
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Load More for Cursor Pagination -->
    <div v-if="fileStore.hasMore && !fileStore.loading" class="mt-6 text-center">
      <button
        @click.stop="fileStore.fetchNextPage()"
        :disabled="fileStore.loadingMore"
        class="px-5 py-2 rounded-lg bg-slate-800 hover:bg-slate-700 border border-slate-700 text-indigo-300 hover:text-white text-xs font-medium transition cursor-pointer disabled:opacity-50 inline-flex items-center space-x-2 shadow-sm"
      >
        <div v-if="fileStore.loadingMore" class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-indigo-400 border-t-transparent"></div>
        <span>{{ fileStore.loadingMore ? 'Loading more...' : 'Load More Files' }}</span>
        <span v-if="fileStore.totalCount" class="text-slate-500 text-[10px]">({{ filteredEntries.length }} of {{ fileStore.totalCount }})</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import { useVirtualizer } from '@tanstack/vue-virtual';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';
import { getDownloadUrl } from '../../api/files';
import type { FileEntry } from '../../types/vfs';

const fileStore = useFileStore();
const uiStore = useUiStore();

const scrollContainerRef = ref<HTMLElement | null>(null);
const filteredEntries = computed<FileEntry[]>(() => fileStore.filteredEntries);

// ── Responsive column count ─────────────────────────────────────────────────
const containerWidth = ref(0);

const cols = computed(() => {
  const w = containerWidth.value;
  if (w >= 1280) return 8;       // xl
  if (w >= 1024) return 6;       // lg
  if (w >= 768)  return 4;       // md
  if (w >= 640)  return 3;       // sm
  return 2;
});

const gridClass = computed(() => {
  const c = cols.value;
  return `grid-cols-${c}`;
});

// Observe container width changes
let resizeObserver: ResizeObserver | null = null;
onMounted(() => {
  if (scrollContainerRef.value) {
    containerWidth.value = scrollContainerRef.value.offsetWidth;
    resizeObserver = new ResizeObserver((entries) => {
      containerWidth.value = entries[0]?.contentRect.width ?? 0;
    });
    resizeObserver.observe(scrollContainerRef.value);
  }
});
onBeforeUnmount(() => resizeObserver?.disconnect());

// ── Virtual row count = ceil(entries / cols) ────────────────────────────────
const rowCount = computed(() => Math.ceil(filteredEntries.value.length / cols.value));

const rowVirtualizer = useVirtualizer({
  get count() { return rowCount.value; },
  getScrollElement: () => scrollContainerRef.value,
  estimateSize: () => 110,   // estimated cell height (icon + text)
  overscan: 3,
});

const virtualItems = computed(() => rowVirtualizer.value.getVirtualItems());
const totalVirtualSize = computed(() => rowVirtualizer.value.getTotalSize());

/** Returns the entry at (rowIndex * cols + colOffset), or null if beyond range */
function rowEntries(rowIndex: number, colOffset: number): FileEntry | null {
  return filteredEntries.value[rowIndex * cols.value + colOffset] ?? null;
}

// ── Interaction ─────────────────────────────────────────────────────────────
function handleClick(e: MouseEvent, entry: FileEntry) {
  fileStore.toggleSelect(entry.path, e.ctrlKey || e.metaKey);
}

function handleDoubleClick(entry: FileEntry) {
  if (entry.kind === 'directory') {
    fileStore.navigateTo(entry.path);
  } else {
    const url = getDownloadUrl(fileStore.currentConnectionId, entry.path);
    window.open(url, '_blank');
  }
}

// ── Formatters ───────────────────────────────────────────────────────────────
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}
</script>
