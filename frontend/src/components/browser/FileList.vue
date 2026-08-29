<template>
  <!-- Scroll container: virtualizer needs a fixed-height parent with overflow-auto -->
  <div
    ref="scrollContainerRef"
    class="flex-1 overflow-auto bg-slate-950 select-none text-xs text-slate-300 relative"
  >
    <table class="w-full text-left border-collapse">
      <!-- Sticky Header -->
      <thead class="bg-slate-900/80 sticky top-0 z-10 border-b border-slate-800 text-[11px] text-slate-400 font-semibold uppercase tracking-wider">
        <tr>
          <th class="py-2.5 px-3 w-10 text-center">
            <input
              type="checkbox"
              :checked="isAllSelected"
              @change="toggleSelectAll"
              class="rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-0 cursor-pointer"
            />
          </th>
          <th class="py-2.5 px-3 cursor-pointer hover:text-white transition" @click="setSort('name')">
            Name <span v-if="fileStore.sortField === 'name'">{{ fileStore.sortOrder === 'asc' ? '↑' : '↓' }}</span>
          </th>
          <th class="py-2.5 px-3 w-28 text-right cursor-pointer hover:text-white transition" @click="setSort('size')">
            Size <span v-if="fileStore.sortField === 'size'">{{ fileStore.sortOrder === 'asc' ? '↑' : '↓' }}</span>
          </th>
          <th class="py-2.5 px-3 w-40 cursor-pointer hover:text-white transition" @click="setSort('modified')">
            Modified <span v-if="fileStore.sortField === 'modified'">{{ fileStore.sortOrder === 'asc' ? '↑' : '↓' }}</span>
          </th>
          <th class="py-2.5 px-3 w-24 text-center">Perms</th>
        </tr>
      </thead>

      <tbody>
        <!-- Empty State -->
        <tr v-if="filteredEntries.length === 0 && !fileStore.loading">
          <td colspan="5" class="py-12 text-center text-slate-600">
            <p class="text-sm">Folder is empty</p>
            <p class="text-[11px] text-slate-700 mt-1">Upload or create a file to get started</p>
          </td>
        </tr>

        <!-- Loading State -->
        <tr v-if="fileStore.loading">
          <td colspan="5" class="py-12 text-center text-slate-500">
            <div class="inline-block animate-spin rounded-full h-5 w-5 border-2 border-indigo-500 border-t-transparent mb-2"></div>
            <p>Loading files...</p>
          </td>
        </tr>

        <!-- Virtual spacer TOP (above rendered window) -->
        <tr v-if="!fileStore.loading && virtualItems.length > 0" aria-hidden="true">
          <td colspan="5" :style="{ height: `${offsetTop}px`, padding: 0 }"></td>
        </tr>

        <!-- Virtualised Rows — only ~50 DOM rows at a time regardless of total entries -->
        <tr
          v-for="vRow in virtualItems"
          :key="filteredEntries[vRow.index]?.path ?? vRow.index"
          :ref="el => measureRow(el as HTMLElement | null, vRow.index)"
          @click="handleClick($event, filteredEntries[vRow.index])"
          @dblclick="handleDoubleClick(filteredEntries[vRow.index])"
          @contextmenu="uiStore.openContextMenu($event, filteredEntries[vRow.index])"
          :class="[
            'cursor-pointer transition group',
            fileStore.selectedEntries.includes(filteredEntries[vRow.index]?.path)
              ? 'bg-indigo-600/15 text-indigo-200'
              : 'hover:bg-slate-900/60'
          ]"
        >
          <!-- Select Box -->
          <td class="py-2 px-3 text-center" @click.stop>
            <input
              type="checkbox"
              :checked="fileStore.selectedEntries.includes(filteredEntries[vRow.index]?.path)"
              @change="fileStore.toggleSelect(filteredEntries[vRow.index]?.path, true)"
              class="rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-0 cursor-pointer"
            />
          </td>

          <!-- File / Folder Name & Icon -->
          <td class="py-2 px-3 flex items-center space-x-2.5 font-medium truncate">
            <span class="text-base leading-none">
              <span v-if="filteredEntries[vRow.index]?.kind === 'directory'">📁</span>
              <span v-else-if="filteredEntries[vRow.index]?.kind === 'symlink'">🔗</span>
              <span v-else>📄</span>
            </span>
            <span
              class="truncate"
              :class="{ 'text-indigo-300 font-semibold': filteredEntries[vRow.index]?.kind === 'directory' }"
            >
              {{ filteredEntries[vRow.index]?.name }}
            </span>
          </td>

          <!-- Size -->
          <td class="py-2 px-3 text-right text-slate-400 font-mono text-[11px]">
            {{ filteredEntries[vRow.index]?.kind === 'directory' ? '-' : formatBytes(filteredEntries[vRow.index]?.size || 0) }}
          </td>

          <!-- Modified Date -->
          <td class="py-2 px-3 text-slate-500 text-[11px] truncate">
            {{ formatDate(filteredEntries[vRow.index]?.modified_at) }}
          </td>

          <!-- Permissions -->
          <td class="py-2 px-3 text-center text-slate-500 font-mono text-[10px]">
            {{ filteredEntries[vRow.index]?.permissions || '-' }}
          </td>
        </tr>

        <!-- Virtual spacer BOTTOM (below rendered window) -->
        <tr v-if="!fileStore.loading && virtualItems.length > 0" aria-hidden="true">
          <td colspan="5" :style="{ height: `${offsetBottom}px`, padding: 0 }"></td>
        </tr>

        <!-- Load More Row for Cursor Pagination (appears below virtualised list) -->
        <tr v-if="fileStore.hasMore && !fileStore.loading" class="hover:bg-slate-900/40">
          <td colspan="5" class="py-3 text-center">
            <button
              @click.stop="fileStore.fetchNextPage()"
              :disabled="fileStore.loadingMore"
              class="px-4 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-indigo-300 hover:text-white text-xs font-medium transition cursor-pointer disabled:opacity-50 inline-flex items-center space-x-2"
            >
              <div v-if="fileStore.loadingMore" class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-indigo-400 border-t-transparent"></div>
              <span>{{ fileStore.loadingMore ? 'Loading more...' : 'Load More Files' }}</span>
              <span v-if="fileStore.totalCount" class="text-slate-500 text-[10px]">({{ filteredEntries.length }} of {{ fileStore.totalCount }})</span>
            </button>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, shallowRef } from 'vue';
import { useVirtualizer } from '@tanstack/vue-virtual';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';
import { getDownloadUrl } from '../../api/files';
import type { FileEntry } from '../../types/vfs';

const fileStore = useFileStore();
const uiStore = useUiStore();

// Scroll container reference for the virtualizer
const scrollContainerRef = ref<HTMLElement | null>(null);

// Use the already-filtered entries from fileStore
const filteredEntries = computed<FileEntry[]>(() => fileStore.filteredEntries);

// Row element refs for dynamic measurement (handles variable-height rows)
const rowRefs = shallowRef<Map<number, HTMLElement>>(new Map());
function measureRow(el: HTMLElement | null, index: number) {
  if (el) rowRefs.value.set(index, el);
  else rowRefs.value.delete(index);
}

// TanStack virtual list — renders only the visible ~50-80 rows
const rowVirtualizer = useVirtualizer({
  get count() { return filteredEntries.value.length; },
  getScrollElement: () => scrollContainerRef.value,
  estimateSize: () => 40,   // estimated row height in px
  overscan: 10,             // extra rows above/below viewport for smooth scroll
});

const virtualItems = computed(() => rowVirtualizer.value.getVirtualItems());
const totalVirtualSize = computed(() => rowVirtualizer.value.getTotalSize());

// Spacer heights to maintain correct scroll position
const offsetTop = computed(() => virtualItems.value[0]?.start ?? 0);
const offsetBottom = computed(() => {
  const last = virtualItems.value.at(-1);
  if (!last) return 0;
  return totalVirtualSize.value - last.end;
});

// --- Selection ---
const isAllSelected = computed(() => {
  return (
    filteredEntries.value.length > 0 &&
    fileStore.selectedEntries.length === filteredEntries.value.length
  );
});

function toggleSelectAll() {
  if (isAllSelected.value) {
    fileStore.clearSelection();
  } else {
    fileStore.selectAll();
  }
}

// --- Sorting ---
function setSort(field: string) {
  if (fileStore.sortField === field) {
    fileStore.sortOrder = fileStore.sortOrder === 'asc' ? 'desc' : 'asc';
  } else {
    fileStore.sortField = field as any;
    fileStore.sortOrder = 'asc';
  }
  fileStore.fetchEntries();
}

// --- Interaction ---
function handleClick(e: MouseEvent, entry: FileEntry) {
  if (!entry) return;
  fileStore.toggleSelect(entry.path, e.ctrlKey || e.metaKey);
}

function handleDoubleClick(entry: FileEntry) {
  if (!entry) return;
  if (entry.kind === 'directory') {
    fileStore.navigateTo(entry.path);
  } else {
    const url = getDownloadUrl(fileStore.currentConnectionId, entry.path);
    window.open(url, '_blank');
  }
}

// --- Formatters ---
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatDate(dateStr?: string): string {
  if (!dateStr) return '-';
  const d = new Date(dateStr);
  return d.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
</script>
